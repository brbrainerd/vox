//! Economy SSOT loader.
//!
//! Externalizes the numeric reward/tuning constants that are otherwise baked
//! into [`crate::reward_policy`] and [`crate::streak`] into a single data file
//! (`contracts/gamify/economy.v1.yaml`). The hard-coded values in those modules
//! remain the canonical in-code default (the fallback SSOT); this loader
//! overlays a YAML file **on top of** those defaults.
//!
//! ## Overlay semantics
//! - Any key omitted from the YAML keeps its hard-coded default.
//! - An absent or empty file is therefore behavior-preserving (pure defaults).
//! - The shipped contract carries values IDENTICAL to the defaults, so loading
//!   it changes nothing — a regression test asserts this equality, proving the
//!   file is consumed and behavior is preserved.
//!
//! This composes with the existing DB-driven [`crate::reward_policy::EventConfigOverrides`]
//! rather than bypassing it: load the economy file into an [`EconomyConfig`],
//! then materialize per-event overrides via [`EconomyConfig::to_overrides`].

use crate::reward_policy::{BaseReward, EventConfigOverrides};
use std::collections::HashMap;
use std::path::Path;

/// Scalar tuning constants. Defaults mirror the `reward_policy`/`streak` constants.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    /// Occurrences after which a session reward tapers to 0.1x (`GRIND_TAPER_END`).
    pub grind_taper_end: u32,
    /// Occurrence count past which a reward is fully suppressed (`GRIND_ZERO_THRESHOLD`).
    pub grind_zero_threshold: u32,
    /// `(full_cap, half_cap)` grind tiers for high-frequency events.
    pub grind_caps_high_frequency: (u32, u32),
    /// `(full_cap, half_cap)` grind tiers for default events.
    pub grind_caps_default: (u32, u32),
    /// Novelty multiplier on first occurrence (`NOVELTY_FACTOR`).
    pub novelty_factor: f64,
    /// Streak bonus added per day.
    pub streak_bonus_per_day: f64,
    /// Streak-day cap for the bonus.
    pub streak_bonus_cap_days: u32,
    /// Learning-mode crystal jitter modulus.
    pub learning_jitter_modulus: u64,
    /// Daily login streak base bonus XP.
    pub daily_streak_base_bonus: u64,
    /// Daily login streak bonus XP cap.
    pub daily_streak_bonus_cap: u64,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            grind_taper_end: crate::reward_policy::GRIND_TAPER_END,
            grind_zero_threshold: crate::reward_policy::GRIND_ZERO_THRESHOLD,
            grind_caps_high_frequency: (8, 14),
            grind_caps_default: (15, 25),
            novelty_factor: 1.5,
            streak_bonus_per_day: 0.02,
            streak_bonus_cap_days: 25,
            learning_jitter_modulus: 4,
            daily_streak_base_bonus: 10,
            daily_streak_bonus_cap: 100,
        }
    }
}

/// Trust-tier reward multipliers. Defaults mirror `trust_tier_multiplier`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrustTierMultipliers {
    /// Novice multiplier.
    pub novice: f64,
    /// Linked multiplier.
    pub linked: f64,
    /// Proven multiplier.
    pub proven: f64,
    /// Master multiplier.
    pub master: f64,
}

impl Default for TrustTierMultipliers {
    fn default() -> Self {
        Self {
            novice: 0.5,
            linked: 1.0,
            proven: 1.2,
            master: 1.5,
        }
    }
}

impl TrustTierMultipliers {
    /// Resolve the multiplier for a given trust tier.
    pub fn multiplier(&self, tier: crate::profile::TrustTier) -> f64 {
        match tier {
            crate::profile::TrustTier::Novice => self.novice,
            crate::profile::TrustTier::Linked => self.linked,
            crate::profile::TrustTier::Proven => self.proven,
            crate::profile::TrustTier::Master => self.master,
        }
    }
}

/// Fully-resolved economy configuration: scalar tuning + trust tiers + the
/// reward table, with all YAML overrides overlaid onto the in-code defaults.
#[derive(Debug, Clone, Default)]
pub struct EconomyConfig {
    /// Scalar anti-grind / streak / novelty constants.
    pub tuning: Tuning,
    /// Trust-tier reward multipliers.
    pub trust_tier_multipliers: TrustTierMultipliers,
    /// Reward-table overrides (event_type → base reward). Events not present
    /// here fall back to [`crate::reward_policy::base_reward`].
    pub rewards: HashMap<String, BaseReward>,
}

impl EconomyConfig {
    /// Resolve the effective base reward for an event, applying any reward-table
    /// override on top of the hard-coded [`crate::reward_policy::base_reward`].
    pub fn base_reward(&self, event_type: &str) -> BaseReward {
        self.rewards
            .get(event_type)
            .cloned()
            .unwrap_or_else(|| crate::reward_policy::base_reward(event_type))
    }

    /// Materialize the reward table into [`EventConfigOverrides`] so the economy
    /// file composes with the existing DB-override pathway (DB overrides, applied
    /// afterward, still win — this only seeds the file-driven layer).
    pub fn to_overrides(&self) -> EventConfigOverrides {
        let mut ov = EventConfigOverrides::default();
        for (event, reward) in &self.rewards {
            ov.set(event.clone(), reward.xp, reward.crystals);
        }
        ov
    }
}

// ── On-disk schema (serde) ────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct RawEconomy {
    #[serde(default)]
    tuning: Option<RawTuning>,
    #[serde(default)]
    trust_tier_multipliers: Option<RawTrust>,
    #[serde(default)]
    rewards: HashMap<String, RawReward>,
}

#[derive(serde::Deserialize)]
struct RawTuning {
    grind_taper_end: Option<u32>,
    grind_zero_threshold: Option<u32>,
    grind_caps_high_frequency: Option<(u32, u32)>,
    grind_caps_default: Option<(u32, u32)>,
    novelty_factor: Option<f64>,
    streak_bonus_per_day: Option<f64>,
    streak_bonus_cap_days: Option<u32>,
    learning_jitter_modulus: Option<u64>,
    daily_streak_base_bonus: Option<u64>,
    daily_streak_bonus_cap: Option<u64>,
}

#[derive(serde::Deserialize)]
struct RawTrust {
    novice: Option<f64>,
    linked: Option<f64>,
    proven: Option<f64>,
    master: Option<f64>,
}

#[derive(serde::Deserialize)]
struct RawReward {
    xp: u64,
    crystals: u64,
    #[serde(default)]
    lumens: i64,
    #[serde(default)]
    grant_shield: bool,
}

/// Load and overlay an economy file onto the in-code defaults.
///
/// A missing key keeps its default; an absent file is a hard error (callers that
/// want "default if absent" should check existence first or call
/// [`EconomyConfig::default`]). Parsing failures return a descriptive error.
pub fn load_economy(path: impl AsRef<Path>) -> anyhow::Result<EconomyConfig> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading economy file {}: {e}", path.display()))?;
    parse_economy(&text)
        .map_err(|e| anyhow::anyhow!("parsing economy file {}: {e}", path.display()))
}

/// Parse an economy YAML string, overlaying onto in-code defaults.
pub fn parse_economy(text: &str) -> anyhow::Result<EconomyConfig> {
    let raw: RawEconomy = serde_yaml::from_str(text)?;
    let mut cfg = EconomyConfig::default();

    if let Some(t) = raw.tuning {
        let d = &mut cfg.tuning;
        if let Some(v) = t.grind_taper_end {
            d.grind_taper_end = v;
        }
        if let Some(v) = t.grind_zero_threshold {
            d.grind_zero_threshold = v;
        }
        if let Some(v) = t.grind_caps_high_frequency {
            d.grind_caps_high_frequency = v;
        }
        if let Some(v) = t.grind_caps_default {
            d.grind_caps_default = v;
        }
        if let Some(v) = t.novelty_factor {
            d.novelty_factor = v;
        }
        if let Some(v) = t.streak_bonus_per_day {
            d.streak_bonus_per_day = v;
        }
        if let Some(v) = t.streak_bonus_cap_days {
            d.streak_bonus_cap_days = v;
        }
        if let Some(v) = t.learning_jitter_modulus {
            d.learning_jitter_modulus = v;
        }
        if let Some(v) = t.daily_streak_base_bonus {
            d.daily_streak_base_bonus = v;
        }
        if let Some(v) = t.daily_streak_bonus_cap {
            d.daily_streak_bonus_cap = v;
        }
    }

    if let Some(t) = raw.trust_tier_multipliers {
        let d = &mut cfg.trust_tier_multipliers;
        if let Some(v) = t.novice {
            d.novice = v;
        }
        if let Some(v) = t.linked {
            d.linked = v;
        }
        if let Some(v) = t.proven {
            d.proven = v;
        }
        if let Some(v) = t.master {
            d.master = v;
        }
    }

    for (event, r) in raw.rewards {
        cfg.rewards.insert(
            event,
            BaseReward {
                xp: r.xp,
                crystals: r.crystals,
                lumens: r.lumens,
                grant_shield: r.grant_shield,
            },
        );
    }

    Ok(cfg)
}

/// Repo-relative path to the shipped economy contract, resolved from this crate.
pub const SHIPPED_CONTRACT_RELPATH: &str = "../../contracts/gamify/economy.v1.yaml";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::TrustTier;
    use std::path::PathBuf;

    fn shipped_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SHIPPED_CONTRACT_RELPATH)
    }

    fn shipped() -> EconomyConfig {
        load_economy(shipped_path()).expect("shipped economy contract loads")
    }

    /// Behavior preservation: the shipped contract's scalar tuning EQUALS the
    /// in-code defaults (which mirror the reward_policy/streak constants).
    #[test]
    fn shipped_tuning_equals_in_code_defaults() {
        let cfg = shipped();
        assert_eq!(cfg.tuning, Tuning::default());
        // And those defaults are pinned to the actual reward_policy constants.
        assert_eq!(
            cfg.tuning.grind_taper_end,
            crate::reward_policy::GRIND_TAPER_END
        );
        assert_eq!(
            cfg.tuning.grind_zero_threshold,
            crate::reward_policy::GRIND_ZERO_THRESHOLD
        );
    }

    /// Behavior preservation: trust multipliers EQUAL `trust_tier_multiplier`.
    #[test]
    fn shipped_trust_multipliers_equal_in_code() {
        let cfg = shipped();
        for tier in [
            TrustTier::Novice,
            TrustTier::Linked,
            TrustTier::Proven,
            TrustTier::Master,
        ] {
            assert_eq!(
                cfg.trust_tier_multipliers.multiplier(tier),
                crate::reward_policy::trust_tier_multiplier(tier),
                "trust multiplier mismatch for {tier:?}"
            );
        }
    }

    /// Behavior preservation: EVERY event in `base_reward`'s table round-trips
    /// to an identical reward when resolved through the loaded economy config.
    #[test]
    fn shipped_rewards_equal_base_reward_table() {
        let cfg = shipped();
        // The full list of known event types baked into base_reward().
        let events = [
            "task_completed",
            "task_started",
            "task_submitted",
            "task_failed",
            "task_doubted",
            "task_resolved",
            "agent_spawned",
            "agent_retired",
            "agent_idle",
            "agent_busy",
            "snapshot_captured",
            "operation_undone",
            "operation_redone",
            "conflict_resolved",
            "plan_handoff",
            "agent_handoff_accepted",
            "peer_teach_session",
            "message_sent",
            "pr_merged",
            "code_reviewed",
            "issue_closed",
            "helped_peer",
            "bounty_completed",
            "refactor",
            "bug_fix",
            "test_pass",
            "lint_clean",
            "doc_added",
            "build_completed",
            "build_failed",
            "check_completed",
            "check_failed",
            "test_fail",
            "fmt_completed",
            "cli_command_completed",
            "cli_command_failed",
            "diagnostics_clean",
            "completion_accepted",
            "bundle_completed",
            "build_clean",
            "build_failed_then_fixed",
            "phoenix_bonus",
            "build_clean_streak_3",
            "check_clean_first_try",
            "test_suite_green",
            "test_coverage_improved",
            "toestub_violations_fixed",
            "toestub_scan_clean",
            "stub_check_debt",
            "fmt_applied",
            "doc_coverage_100_pct",
            "missing_docs_zero",
            "ai_thumbs_up",
            "ai_thumbs_down",
            "ai_example_written",
            "ai_example_accepted",
            "populi_corpus_contributed",
            "populi_inference_run",
            "populi_finetune_epoch",
            "mens_flywheel_triggered",
            "vox_example_created",
            "vox_example_canonical",
            "migration_applied",
            "seed_completed",
            "vox_web_page_rendered",
            "v0_import_complete",
            "lsp_go_to_def_used",
            "lsp_completion_accepted",
            "openapi_spec_generated",
            "scheduled_job_ran",
            "turso_query_executed",
            "mcp_tool_called",
            "mcp_tool_registered",
            "pkg_published",
            "pkg_installed",
            "workflow_completed",
            "workflow_checkpoint_saved",
            "actor_message_sent",
            "actor_spawned",
            "security_review_passed",
            "perf_regression_caught",
            "unsafe_removed",
            "collegium_created",
            "collegium_joined",
            "arena_joined",
            "virtus_trifecta",
            "exterminatus",
            "iron_will_recovery",
            "scribes_fury",
            "review_fix_ship_bonus",
            "cost_incurred",
            "continuation_triggered",
            "scope_violation",
        ];
        for ev in events {
            let baked = crate::reward_policy::base_reward(ev);
            let loaded = cfg.base_reward(ev);
            assert_eq!(
                (
                    loaded.xp,
                    loaded.crystals,
                    loaded.lumens,
                    loaded.grant_shield
                ),
                (baked.xp, baked.crystals, baked.lumens, baked.grant_shield),
                "economy reward mismatch for event '{ev}'"
            );
        }
    }

    /// Unknown event falls back to the policy base (here: zero reward).
    #[test]
    fn unknown_event_falls_back_to_policy_base() {
        let cfg = shipped();
        let r = cfg.base_reward("totally_unknown_event_xyz");
        assert_eq!((r.xp, r.crystals), (0, 0));
    }

    /// Partial / absent overlay is safe: empty YAML yields pure defaults.
    #[test]
    fn empty_overlay_yields_defaults() {
        let cfg = parse_economy("{}").expect("empty doc parses");
        assert_eq!(cfg.tuning, Tuning::default());
        assert_eq!(cfg.trust_tier_multipliers, TrustTierMultipliers::default());
        assert!(cfg.rewards.is_empty());
        // Falls through to baked table for a known event.
        let r = cfg.base_reward("task_completed");
        assert_eq!((r.xp, r.crystals), (50, 5));
    }

    /// Partial overlay: a single tuning key changes only that key.
    #[test]
    fn partial_tuning_overlay_keeps_other_defaults() {
        let cfg = parse_economy("tuning:\n  novelty_factor: 2.0\n").unwrap();
        assert_eq!(cfg.tuning.novelty_factor, 2.0);
        assert_eq!(
            cfg.tuning.grind_taper_end,
            Tuning::default().grind_taper_end
        );
    }

    /// The reward table composes into EventConfigOverrides for the DB pathway.
    #[test]
    fn to_overrides_resolves_a_known_event() {
        let cfg = shipped();
        let ov = cfg.to_overrides();
        let r = ov.resolve("bug_fix");
        assert_eq!((r.xp, r.crystals), (200, 40));
    }
}
