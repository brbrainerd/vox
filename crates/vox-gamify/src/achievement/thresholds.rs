//! Counter name → achievement id thresholds for unlock checks.

pub(super) fn for_counter(counter: &str) -> Vec<(&'static str, u32)> {
    match counter {
        "tasks_completed" => vec![
            ("first_task", 1),
            ("five_tasks", 5),
            ("twenty_five_tasks", 25),
            ("hundred_tasks", 100),
            ("five_hundred_tasks", 500),
        ],
        "tasks_today" => vec![("task_in_a_day", 10)],
        "tasks_submitted" => vec![("first_task_queued", 1), ("steady_queue", 25)],
        "workspace_snapshots" => vec![("first_checkpoint", 1), ("checkpoint_habit", 50)],
        "inter_agent_messages" => vec![("first_wave", 1), ("signal_traffic", 100)],
        "vcs_locks_acquired" => vec![("first_exclusive_lock", 1), ("locksmith", 200)],
        "handoffs_completed" => vec![
            ("first_handoff", 1),
            ("ten_handoffs", 10),
            ("fifty_handoffs", 50),
        ],
        "vcs_conflicts_resolved" => vec![("conflict_resolver", 1)],
        "error_free_streak" => vec![("error_free_five", 5)],
        "continuations_received" => vec![("first_continuation", 1)],
        "activity_streak" => vec![
            ("streak_3", 3),
            ("streak_7", 7),
            ("streak_14", 14),
            ("streak_30", 30),
            ("streak_90", 90),
            ("streak_365", 365),
        ],
        "challenges_solved" => vec![("challenge_solved", 1)],
        "memory_entries" => vec![("first_memory", 1)],
        "langs_used" => vec![("polyglot", 5), ("polyglot_10", 10)],
        // AI Corpus
        "ai_feedback_given" => vec![
            ("first_thumbs", 1),
            ("ten_thumbs", 10),
            ("hundred_thumbs", 100),
        ],
        "ai_negative_feedback_given" => vec![("first_thumbs_down", 1)],
        "ai_positive_feedback_given" => vec![("first_thumbs", 1)],
        "vox_examples_written" => vec![("first_vox_example", 1), ("five_vox_examples", 5)],
        "canonical_examples" => vec![("canonical_example", 1)],
        "corpus_contributions" => vec![
            ("first_corpus_contribution", 1),
            ("ten_corpus_contributions", 10),
        ],
        "finetune_epochs" => vec![("first_finetune", 1)],
        "inference_runs" => vec![("inference_regular", 50)],
        // Build Mastery
        "green_builds" => vec![("first_green_build", 1)],
        "consecutive_green_builds" => vec![
            ("build_streak_3", 3),
            ("build_streak_10", 10),
            ("build_streak_30", 30),
        ],
        "builds_fixed" => vec![("first_fix", 1)],
        "zero_warning_checks" => vec![("zero_warnings", 1)],
        "toestub_clean_crates" => vec![("toestub_clean_crate", 1)],
        "toestub_workspace_clean" => vec![("toestub_clean_workspace", 1)],
        // Documentation
        "doc_comments_added" => vec![
            ("first_doc", 1),
            ("fifty_docs", 50),
            ("five_hundred_docs", 500),
        ],
        "crates_doc_clean" => vec![("crate_doc_clean", 1)],
        "workspace_doc_clean" => vec![("workspace_doc_clean", 1)],
        "research_docs_written" => vec![("research_doc_written", 1)],
        "adrs_written" => vec![("adr_written", 1), ("first_adr", 1)],
        // Language Explorer
        "vox_web_pages_compiled" => vec![("first_vox_web_page", 1)],
        "migrations_applied" => vec![("first_migration", 1)],
        "seeds_run" => vec![("first_seed", 1)],
        "workflows_completed" => vec![("first_workflow", 1)],
        "actors_spawned" => vec![("first_actor", 1)],
        "mcp_tools_registered" => vec![("first_mcp_tool", 1)],
        "openapi_specs_generated" => vec![("first_openapi", 1)],
        "packages_published" => vec![("first_pkg_publish", 1)],
        "v0_imports" => vec![("first_v0_import", 1)],
        "scheduled_jobs_run" => vec![("first_scheduled_job", 1)],
        "turso_queries" => vec![("first_turso_query", 1)],
        "populi_serves" => vec![("first_populi_serve", 1)],
        // Efficiency
        "fast_tasks_30s" => vec![("speed_demon", 1)],
        "fast_tasks_10s" => vec![("ultra_speed", 1)],
        "zero_cost_sessions" => vec![("zero_cost_session", 1)],
        "offline_sessions" => vec![("offline_session", 1)],
        // Security
        "security_reviews_passed" => vec![("first_security_pass", 1)],
        "null_clean_scans" => vec![("no_null_violations", 1)],
        "perf_regressions_caught" => vec![("perf_regression_caught", 1)],
        "unsafe_blocks_removed" => vec![("first_unsafe_removed", 1)],
        // Build mastery (new)
        "toestub_violations_fixed" => vec![("first_toestub_fix", 1)],
        "tests_written" => vec![("first_test_written", 1)],
        // Research / docs (new)
        "research_urls_ingested" => vec![("first_research_ingest", 1)],
        // Battle (new)
        "battles_entered" => vec![("first_bug_battle", 1)],
        "battles_won" => vec![
            ("first_battle_won", 1),
            ("victor_five", 5),
            ("victor_twenty", 20),
        ],
        // Corpus (new)
        "training_pairs_rated" => vec![("first_corpus_rating", 1)],
        // Collaboration (new)
        "handoffs_received" => vec![("first_handoff_received", 1)],
        "students_taught" => vec![("first_peer_teach", 1), ("ten_students_taught", 10)],
        "prs_merged" => vec![("vox_foundation_contributor", 1)],
        // Daily quests (new)
        "daily_quests_completed" => vec![("first_daily_quest", 1)],
        "daily_quest_streak" => vec![
            ("daily_quest_streak_3", 3),
            ("daily_quest_streak_7", 7),
            ("daily_quest_streak_30", 30),
        ],
        "perfect_daily_completions" => vec![("perfect_daily_3", 1)],
        "perfect_weeks" => vec![("perfect_week", 1)],
        "legendary_quests_completed" => vec![("legendary_quest_complete", 1)],
        "chains_quest_pairs_completed" => vec![("chains_quest_complete", 1)],
        // Level milestones (new) — caller sets counter to current level
        "player_level" => vec![
            ("reach_level_10", 10),
            ("reach_level_25", 25),
            ("reach_level_50", 50),
            ("reach_level_100", 100),
            ("reach_level_200", 200),
            ("reach_level_500", 500),
            ("reach_level_1000", 1000),
        ],
        // Prestige (new)
        "prestige_count" => vec![
            ("first_prestige", 1),
            ("prestige_5", 5),
            ("prestige_10", 10),
        ],
        // Lifetime XP milestone — caller converts u64→u32 (saturating) for large values
        "lifetime_xp_millions" => vec![("million_lifetime_xp", 1)],
        // Social feedback balance (new)
        "balanced_feedback_weeks" => vec![("balanced_feedback_week", 1)],
        _ => vec![],
    }
}

#[cfg(test)]
mod semcov_wave13_tests {
    use super::*;

    // ── Known counters return non-empty threshold lists ────────────────────

    #[test]
    fn tasks_completed_has_five_thresholds() {
        // Catches: accidentally returning wrong number of thresholds
        let thresholds = for_counter("tasks_completed");
        assert_eq!(
            thresholds.len(),
            5,
            "expected 5 thresholds for tasks_completed"
        );
    }

    #[test]
    fn tasks_completed_first_threshold_is_one() {
        // Catches: first threshold > 1 (meaning first event never unlocks)
        let thresholds = for_counter("tasks_completed");
        let first = thresholds[0];
        assert_eq!(first.0, "first_task");
        assert_eq!(first.1, 1);
    }

    #[test]
    fn tasks_completed_thresholds_strictly_ascending() {
        // Catches: thresholds out of order causing premature or missed unlocks
        let thresholds = for_counter("tasks_completed");
        let values: Vec<u32> = thresholds.iter().map(|(_, v)| *v).collect();
        for window in values.windows(2) {
            assert!(
                window[0] < window[1],
                "thresholds must be strictly ascending: {:?}",
                values
            );
        }
    }

    #[test]
    fn activity_streak_has_six_thresholds_including_365() {
        // Catches: accidental deletion of year streak threshold
        let thresholds = for_counter("activity_streak");
        assert_eq!(thresholds.len(), 6);
        let values: Vec<u32> = thresholds.iter().map(|(_, v)| *v).collect();
        assert!(values.contains(&365), "365-day streak threshold must exist");
    }

    #[test]
    fn activity_streak_thresholds_start_at_3() {
        // Catches: first streak threshold set to 1 or 2 (wrong milestone)
        let thresholds = for_counter("activity_streak");
        assert_eq!(thresholds[0].1, 3, "first streak threshold should be 3");
    }

    #[test]
    fn unknown_counter_returns_empty_vec() {
        // Catches: _ wildcard branch returning some default thresholds
        let thresholds = for_counter("this_counter_does_not_exist_xyz");
        assert!(
            thresholds.is_empty(),
            "unknown counter must return empty vec"
        );
    }

    #[test]
    fn build_streak_3_10_30_thresholds_all_present() {
        // Catches: typos in consecutive_green_builds arm that drop a threshold
        let thresholds = for_counter("consecutive_green_builds");
        let values: Vec<u32> = thresholds.iter().map(|(_, v)| *v).collect();
        assert!(values.contains(&3), "missing threshold 3");
        assert!(values.contains(&10), "missing threshold 10");
        assert!(values.contains(&30), "missing threshold 30");
    }

    #[test]
    fn player_level_thresholds_include_milestones_10_through_1000() {
        // Catches: missing or wrong player_level milestone values
        let thresholds = for_counter("player_level");
        let values: Vec<u32> = thresholds.iter().map(|(_, v)| *v).collect();
        for milestone in [10u32, 25, 50, 100, 200, 500, 1000] {
            assert!(
                values.contains(&milestone),
                "player_level missing milestone {milestone}"
            );
        }
    }

    #[test]
    fn battles_won_has_thresholds_1_5_20() {
        // Catches: missing threshold entries in battles_won arm
        let thresholds = for_counter("battles_won");
        let values: Vec<u32> = thresholds.iter().map(|(_, v)| *v).collect();
        assert!(values.contains(&1));
        assert!(values.contains(&5));
        assert!(values.contains(&20));
    }
}
