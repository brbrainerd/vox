//! Semantic-coverage wave-13 adversarial tests for vox-gamify.
//!
//! Targets: leaderboard.rs, reward_policy.rs, profile.rs (XP/level curve), cost.rs
//! Files NOT tested here (covered in wave1d): ability.rs,
//! achievement/defaults/doubt.rs, achievement/defaults/part_a.rs.

use vox_gamify::{
    cost::{CostAggregator, CostSummary},
    db::CostRecord,
    leaderboard::{AgentStats, Leaderboard, LeaderboardMetric},
    profile::{level_from_xp, xp_threshold_for_level, LudusProfile, TrustTier},
    reward_policy::{
        apply_policy, base_reward, trust_tier_multiplier, SessionState, GRIND_ZERO_THRESHOLD,
    },
};

mod semcov_wave13_tests {
    use super::*;

    // ── Leaderboard invariants ──────────────────────────────────────────────────

    #[test]
    fn leaderboard_ranks_are_monotonically_increasing() {
        // Catches: off-by-one in the rank assignment loop that would produce
        // duplicate ranks (e.g. two entries with rank=1) or skip a rank.
        let mut lb = Leaderboard::new();
        for i in 0u32..5 {
            let id = format!("a{i}");
            lb.agent_stats(&id, &id);
            for _ in 0..=(i as usize) {
                lb.record_completion(&id, 1000, 0.01);
            }
        }
        let ranked = lb.ranked(LeaderboardMetric::TasksCompleted);
        assert_eq!(ranked.len(), 5);
        for (i, entry) in ranked.iter().enumerate() {
            assert_eq!(
                entry.rank,
                (i + 1) as u32,
                "rank at position {i} should be {}, got {}",
                i + 1,
                entry.rank
            );
        }
    }

    #[test]
    fn leaderboard_speed_rank_lower_duration_wins() {
        // Catches: sign inversion bug — Speed uses negated avg_duration_ms so
        // that lower duration → higher score → rank 1.  Without the negation
        // the slowest agent would end up at rank 1.
        let mut lb = Leaderboard::new();
        lb.agent_stats("fast", "fast-bot");
        lb.agent_stats("slow", "slow-bot");
        lb.record_completion("fast", 500, 0.01);
        lb.record_completion("slow", 9000, 0.01);

        let ranked = lb.ranked(LeaderboardMetric::Speed);
        assert_eq!(
            ranked[0].agent_id, "fast",
            "faster agent must rank first for Speed metric"
        );
    }

    #[test]
    fn leaderboard_empty_stats_avg_code_quality_defaults_to_50() {
        // Catches: div-by-zero panic when code_quality_count is 0.
        // The implementation uses checked_div with fallback 50; validate the
        // fallback is 50 (not 0 or a panic).
        let stats = AgentStats {
            agent_id: "x".into(),
            agent_name: "x".into(),
            code_quality_sum: 0,
            code_quality_count: 0, // no samples recorded
            ..Default::default()
        };
        assert_eq!(
            stats.avg_code_quality(),
            50,
            "zero-sample code quality should default to 50, not panic or return 0"
        );
    }

    #[test]
    fn leaderboard_reliability_zero_tasks_is_100_pct() {
        // Catches: div-by-zero in reliability_pct when completed + failed == 0.
        // An agent with no history must be considered perfectly reliable.
        let stats = AgentStats::default();
        assert!(
            (stats.reliability_pct() - 100.0).abs() < 1e-9,
            "agent with no tasks must have 100.0% reliability, got {}",
            stats.reliability_pct()
        );
    }

    // ── Reward policy — multiplier boundary ────────────────────────────────────

    #[test]
    fn zero_mode_multiplier_yields_zero_xp_and_crystals() {
        // Catches: a floor being added after the mode multiplier is applied so
        // that "Disabled" mode (0.0x) still leaks nonzero rewards.
        // task_completed has base xp=50, crystals=5.
        let base = base_reward("task_completed");
        let mut session = SessionState::default();
        let r = apply_policy(
            &base,
            0.0, // 0x mode multiplier
            0,
            TrustTier::Linked,
            "task_completed",
            &mut session,
        );
        // novelty (1.5x) * 0.0 mode = 0; both fields must be exactly 0
        assert_eq!(r.xp, 0, "0x mode multiplier must produce 0 XP");
        assert_eq!(r.crystals, 0, "0x mode multiplier must produce 0 crystals");
    }

    #[test]
    fn grind_zero_threshold_exact_boundary() {
        // Catches: off-by-one in the grind cap — at call GRIND_ZERO_THRESHOLD-1
        // rewards must still be positive; at GRIND_ZERO_THRESHOLD they must be 0.
        // Uses "task_completed" (default bucket: full_cap=15, half_cap=25).
        let base = base_reward("task_completed"); // xp=50
        let mut session = SessionState::default();

        // Drive to one call before the zero threshold
        let mut last_xp_before = 0u64;
        for _ in 0..GRIND_ZERO_THRESHOLD - 1 {
            let r = apply_policy(
                &base,
                1.0,
                0,
                TrustTier::Linked,
                "task_completed",
                &mut session,
            );
            last_xp_before = r.xp;
        }
        assert!(
            last_xp_before > 0,
            "call #{} (GRIND_ZERO_THRESHOLD-1) must still yield positive XP",
            GRIND_ZERO_THRESHOLD - 1
        );

        // The GRIND_ZERO_THRESHOLD-th call must be fully capped
        let capped = apply_policy(
            &base,
            1.0,
            0,
            TrustTier::Linked,
            "task_completed",
            &mut session,
        );
        assert!(
            capped.grind_capped,
            "call #{} must set grind_capped=true",
            GRIND_ZERO_THRESHOLD
        );
        assert_eq!(
            capped.xp, 0,
            "call #{} must yield 0 XP",
            GRIND_ZERO_THRESHOLD
        );
    }

    #[test]
    fn taper_threshold_at_full_cap_boundary_for_fast_events() {
        // Catches: fast-event taper bucket (full_cap=8, half_cap=14) being
        // confused with the default (15/25), so snapshot_captured tapers too
        // late.  After 8 calls the 9th must be in the half-cap band (<=50% of
        // base XP = <=15 for snapshot_captured base 30).
        let base = base_reward("snapshot_captured"); // xp=30, crystals=6
        let mut session = SessionState::default();

        // Calls 1-8: full rate — grind_capped must be false on each
        for call in 1u32..=8 {
            let r = apply_policy(
                &base,
                1.0,
                0,
                TrustTier::Linked,
                "snapshot_captured",
                &mut session,
            );
            assert!(
                !r.grind_capped,
                "call {call} for snapshot_captured must not be grind-capped"
            );
        }

        // Call 9: enters 0.5x half-cap band (8 < 9 <= 14)
        let half = apply_policy(
            &base,
            1.0,
            0,
            TrustTier::Linked,
            "snapshot_captured",
            &mut session,
        );
        // base_xp=30, 0.5x grind → expected ≤ 15
        assert!(
            half.xp <= 15,
            "call 9 for snapshot_captured must be tapered to <=15 XP, got {}",
            half.xp
        );
    }

    #[test]
    fn novelty_is_strictly_one_shot() {
        // Catches: seen_types set not being updated on the first record() call,
        // causing the novelty bonus to fire on the second call too.
        // Uses "bug_fix" (xp=200) to make the difference easy to observe.
        let base = base_reward("bug_fix"); // xp=200
        let mut session = SessionState::default();
        let first = apply_policy(&base, 1.0, 0, TrustTier::Linked, "bug_fix", &mut session);
        let second = apply_policy(&base, 1.0, 0, TrustTier::Linked, "bug_fix", &mut session);
        let third = apply_policy(&base, 1.0, 0, TrustTier::Linked, "bug_fix", &mut session);

        assert!(
            first.xp > second.xp,
            "second occurrence must not receive novelty bonus (first={}, second={})",
            first.xp,
            second.xp
        );
        assert_eq!(
            second.xp, third.xp,
            "calls 2 and 3 must yield equal XP — no novelty, same grind band"
        );
    }

    #[test]
    fn trust_tier_master_multiplier_exceeds_novice_and_affects_reward() {
        // Catches: TrustTier multiplier table returning equal values, making
        // tier upgrades give no real benefit; or the multiplier not wired into
        // apply_policy at all.
        let novice_mult = trust_tier_multiplier(TrustTier::Novice);
        let master_mult = trust_tier_multiplier(TrustTier::Master);
        assert!(
            master_mult > novice_mult,
            "Master multiplier ({master_mult}) must exceed Novice ({novice_mult})"
        );

        let base = base_reward("task_completed"); // xp=50
        let mut s1 = SessionState::default();
        let mut s2 = SessionState::default();
        let r_novice = apply_policy(&base, 1.0, 0, TrustTier::Novice, "task_completed", &mut s1);
        let r_master = apply_policy(&base, 1.0, 0, TrustTier::Master, "task_completed", &mut s2);
        assert!(
            r_master.xp > r_novice.xp,
            "Master XP ({}) must exceed Novice XP ({})",
            r_master.xp,
            r_novice.xp
        );
    }

    // ── Profile — XP / level curve invariants ──────────────────────────────────

    #[test]
    fn level_from_xp_is_left_inverse_of_xp_threshold_for_level() {
        // Catches: floating-point rounding in the closed-form inverse causing
        // level_from_xp(xp_threshold_for_level(L)) to return L-1 for some L.
        for level in 1u64..=50 {
            let threshold = xp_threshold_for_level(level);
            let recovered = level_from_xp(threshold);
            assert_eq!(
                recovered, level,
                "level_from_xp(xp_threshold({level})={threshold}) must return {level}, got {recovered}"
            );
        }
    }

    #[test]
    fn xp_threshold_is_strictly_monotonically_increasing() {
        // Catches: arithmetic overflow in the quadratic formula for large levels,
        // which would cause thresholds to wrap around and decrease.
        let mut prev = xp_threshold_for_level(1);
        for level in 2u64..=200 {
            let cur = xp_threshold_for_level(level);
            assert!(
                cur > prev,
                "xp_threshold({level})={cur} must be strictly > xp_threshold({})={prev}",
                level - 1
            );
            prev = cur;
        }
    }

    #[test]
    fn spend_crystals_insufficient_returns_false_and_does_not_mutate() {
        // Catches: u64 underflow wrap-around when spending more crystals than
        // the player has, silently granting a huge crystal balance.
        let mut profile = LudusProfile::new_default("u1");
        let initial = profile.crystals;
        let result = profile.spend_crystals(initial + 1);
        assert!(!result, "spending more than available must return false");
        assert_eq!(
            profile.crystals, initial,
            "failed spend must not mutate crystal balance (would wrap to u64::MAX)"
        );
    }

    // ── Cost aggregator ─────────────────────────────────────────────────────────

    #[test]
    fn budget_alert_fires_strictly_above_80_pct_not_at_exactly_80() {
        // Catches: `>=` instead of `>` in budget_alert comparison — would fire
        // a false alert at exactly 80% spend, annoying users prematurely.
        let mut agg = CostAggregator::new();
        agg.set_budget_limit("bot", 1.0);

        // Spend exactly 80%
        agg.record(CostRecord::new_ephemeral("bot", "openrouter", None, 0, 0, 0.80));
        assert!(
            !agg.budget_alert("bot"),
            "alert must NOT fire at exactly 80% budget usage"
        );

        // Push 1 cent over 80%
        agg.record(CostRecord::new_ephemeral("bot", "openrouter", None, 0, 0, 0.01));
        assert!(
            agg.budget_alert("bot"),
            "alert must fire when spend exceeds 80% of budget"
        );
    }

    #[test]
    fn cost_summary_avg_per_call_zero_calls_returns_zero_not_nan() {
        // Catches: division by zero producing NaN/infinity that silently
        // propagates through downstream float comparisons.
        let summary = CostSummary::default();
        let avg = summary.avg_cost_per_call();
        assert!(avg.is_finite(), "avg_cost_per_call with no calls must be finite");
        assert_eq!(avg, 0.0, "avg_cost_per_call with no calls must be 0.0");
    }
}
