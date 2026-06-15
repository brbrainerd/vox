//! Orchestrator event → gamification rewards (XP, crystals, companion, quests).

use anyhow::Result;
use vox_db::Codex;

use crate::companion::Companion;

use super::collegium::{get_user_collegium, update_collegium_lumens};
use super::companion::{list_companions, upsert_companion};
use super::counters::{increment_counter, set_counter};
use super::notifications::insert_notification;
use super::profile::{
    get_profile, list_unlocked_achievements, record_level_up, unlock_achievement, upsert_profile,
};
use super::quest_battle::{count_quests, list_quests, upsert_quest};
use super::teaching::insert_policy_snapshot;
use crate::notifications::{Notification, NotificationType};

async fn advance_quests(
    db: &Codex,
    profile: &mut crate::profile::LudusProfile,
    user_id: &str,
    quest_type: crate::quest::QuestType,
) {
    if let Ok(mut quests) = list_quests(db, user_id).await {
        for q in quests.iter_mut() {
            if q.quest_type == quest_type && !q.completed {
                if q.increment(1) {
                    profile.add_xp(q.xp_reward);
                    profile.add_crystals(q.crystal_reward);
                }
                let _ = upsert_quest(db, q).await;
            }
        }
    }
}

/// Process an orchestrator event for gamification rewards (XP, crystals, companion stats).
///
/// Handles all `AgentEventKind` variants by delegating companion stat changes to
/// `Companion::interact()` (SSOT) and awarding profile XP/crystals as appropriate.
/// No-ops when gamification is disabled in config.
pub async fn process_event_rewards(
    db: &Codex,
    user_id: &str,
    event_kind: &serde_json::Value,
) -> Result<crate::reward_policy::RouteResult> {
    use crate::companion::Interaction;

    // Early exit when gamify is disabled
    if !crate::config_gate::is_enabled() {
        tracing::trace!("gamify disabled, skipping reward write");
        return Ok(Default::default());
    }

    // 1. Get/Create profile
    let mut profile = match get_profile(db, user_id).await? {
        Some(p) => p,
        None => crate::profile::LudusProfile::new_default(user_id),
    };

    if profile.reward_suppressed {
        tracing::debug!("rewards suppressed for user {}, skipping", user_id);
        return Ok(Default::default());
    }

    // 2. Extract event type and agent info
    //    serde(tag = "type", rename_all = "snake_case") → e.g. "task_completed"
    let event_type = event_kind
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let agent_id = event_kind.get("agent_id").and_then(|v| v.as_u64());
    let agent_id_str = agent_id.map(|id| format!("agent-{}", id));

    // 3. Get/Create companion for the agent involved (if any)
    let mut companion = if let Some(aid) = &agent_id_str {
        list_companions(db, user_id)
            .await?
            .into_iter()
            .find(|c| c.id == *aid)
            .unwrap_or_else(|| Companion::new(aid.clone(), user_id, aid.clone(), "vox"))
    } else {
        Companion::new("_none", user_id, "_none", "vox")
    };

    // 4. Daily Quest Generation (check-in)
    {
        let count = count_quests(db, user_id).await.unwrap_or(0);
        if count == 0 {
            // Generate new daily quests if none active for today
            let daily_quests = crate::quest::todays_quests(user_id);
            for q in daily_quests {
                let _ = upsert_quest(db, &q).await;
            }
        }
    }

    let mut profile_changed = false;
    let mut companion_changed = false;

    // 2b. Update daily streak & detect day change for counters
    let today = crate::quest::current_day_number();
    let last_active_day = profile.last_active as u64 / 86400;
    let streak_res = profile.record_daily_activity();
    if streak_res != crate::streak::StreakResult::AlreadyActive {
        profile_changed = true;

        if today > last_active_day {
            // New day detected: Reset daily counters
            let _ = set_counter(db, user_id, "tasks_today", 0).await;
        }
    }

    // 5. Apply policy-driven rewards
    let mut policy_snapshot: Option<(u64, u64, f64, u64, u64, u32, bool, i64, Option<String>)> =
        None;
    let mut leveled_up_info = None;
    let mut final_reward = None;
    {
        use crate::reward_policy::apply_policy_cfg;
        use std::sync::{Mutex, OnceLock};
        static SESSION: OnceLock<Mutex<crate::reward_policy::SessionState>> = OnceLock::new();
        // Load the economy contract once per process; it is the live source of
        // base rewards and tuning scalars. Missing/invalid file → in-code defaults
        // (behavior-identical to the shipped contract).
        static ECONOMY: OnceLock<crate::economy::EconomyConfig> = OnceLock::new();
        let economy = ECONOMY.get_or_init(crate::economy::resolve_economy);
        let session_lock = SESSION.get_or_init(|| Mutex::new(Default::default()));
        let mode_mult = crate::config_gate::reward_multiplier()
            * crate::config_gate::experiment_reward_multiplier();
        let streak_days = profile.streak.current_streak as u32;
        let mut base_rw = None;
        let mut rw = None;
        if let Ok(mut session) = session_lock.try_lock() {
            // Base reward sourced from the loaded economy contract (falls through
            // to the in-code `base_reward` table for events not overridden).
            let base = economy.base_reward(event_type);
            let reward = apply_policy_cfg(
                economy,
                &base,
                mode_mult,
                streak_days,
                profile.trust_tier,
                event_type,
                &mut session,
            );
            base_rw = Some(base);
            rw = Some(reward);
        }
        if let (Some(base), Some(reward)) = (base_rw, rw) {
            final_reward = Some(reward.clone());
            if reward.xp > 0 {
                let old_level = profile.level;
                let leveled_up = profile.add_xp(reward.xp);
                if leveled_up && profile.level > old_level {
                    leveled_up_info = Some((profile.level, profile.title(), profile.xp));
                }
                profile_changed = true;
            }
            if reward.crystals > 0 {
                profile.add_crystals(reward.crystals);
                profile_changed = true;
            }
            let learn_bonus = crate::reward_policy::learning_mode_crystal_jitter_cfg(
                economy,
                user_id,
                event_type,
                reward.crystals,
            );
            if learn_bonus > 0 {
                profile.add_crystals(learn_bonus);
                profile_changed = true;
            }
            if reward.lumens != 0 {
                profile.add_lumens(reward.lumens);
                profile_changed = true;

                // Aggregate lumens for the player's collegium
                if let Ok(Some((cid, _, _, _))) = get_user_collegium(db, user_id).await {
                    let _ = update_collegium_lumens(db, &cid, reward.lumens).await;
                }
            }
            if reward.grant_shield {
                profile.earn_shield();
                profile_changed = true;
            }
            if reward.xp > 0 || reward.crystals > 0 || reward.lumens != 0 || reward.grant_shield {
                let metadata = event_kind
                    .get("metadata")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                policy_snapshot = Some((
                    base.xp,
                    base.crystals,
                    reward.effective_multiplier,
                    reward.xp,
                    reward.crystals,
                    streak_days,
                    reward.grind_capped,
                    reward.lumens,
                    metadata,
                ));
            }
        }
    }

    // 5b. Record level up (now safe to await outside the sync lock)
    if let Some((lvl, ref title, xp)) = leveled_up_info {
        let _ = record_level_up(db, user_id, lvl, title, xp).await;
        let notif = Notification::new(
            user_id,
            NotificationType::LevelUp,
            format!("Level {lvl}"),
            format!("You reached level {lvl} — {title}"),
        );
        let _ = insert_notification(db, &notif).await;
    }
    if let Some((
        base_xp,
        base_crystals,
        eff_mult,
        rxp,
        rcrystals,
        streak,
        grind_capped,
        rlumens,
        rmetadata,
    )) = policy_snapshot
    {
        let _ = insert_policy_snapshot(
            db,
            user_id,
            event_type,
            base_xp,
            base_crystals,
            &crate::config_gate::policy_snapshot_mode_label(),
            eff_mult,
            rxp,
            rcrystals,
            streak,
            grind_capped,
            rlumens,
            rmetadata.as_deref(),
        )
        .await;
    }

    // 6. Update persistent counters and check achievements
    {
        let counter_names = match event_type {
            "task_completed" => vec!["tasks_completed", "tasks_today"],
            "agent_spawned" => vec!["agents_spawned"],
            "bug_fix" => vec!["bug_fixes"],
            "test_pass" => vec!["tests_passed"],
            "doc_added" => vec!["docs_added"],
            "peer_teach_session" => vec!["peer_teach_sessions"],
            "migration_applied" => vec!["migrations_applied"],
            "seed_completed" => vec!["seeds_run"],
            "v0_import_complete" => vec!["v0_imports"],
            "scheduled_job_ran" => vec!["scheduled_jobs_run"],
            "turso_query_executed" => vec!["turso_queries"],
            "mcp_tool_called" => vec!["mcp_tool_calls"],
            "mcp_tool_registered" => vec!["mcp_tools_registered"],
            "pkg_published" => vec!["packages_published"],
            "workflow_completed" => vec!["workflows_completed"],
            "security_review_passed" => vec!["security_reviews_passed"],
            "perf_regression_caught" => vec!["perf_regressions_caught"],
            "unsafe_removed" => vec!["unsafe_blocks_removed"],
            "ai_thumbs_up" => vec!["ai_feedback_count", "ai_positive_feedback_given"],
            "ai_thumbs_down" => vec!["ai_feedback_count"],
            "ai_example_written" => vec!["ai_examples_written"],
            "populi_corpus_contributed" => vec!["corpus_contributions"],
            "build_clean" => vec!["green_builds"],
            "toestub_violations_fixed" => vec!["toestub_violations_fixed"],
            "toestub_scan_clean" => vec!["toestub_workspace_clean"],
            "finetune_epoch" => vec!["finetune_epochs"],
            "inference_run" => vec!["inference_runs"],
            "daily_quest_completed" => vec!["daily_quests_completed"],
            _ => vec![],
        };

        if !counter_names.is_empty() || profile_changed {
            let mut tracker = crate::achievement::AchievementTracker::new();
            let unlocked_ids: std::collections::HashSet<String> =
                list_unlocked_achievements(db, user_id)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();

            for cname in counter_names {
                let new_val = increment_counter(db, user_id, cname, 1).await.unwrap_or(0);
                let newly_unlocked = tracker.check_unlocks("_current", cname, new_val);
                for ach in newly_unlocked {
                    if !unlocked_ids.contains(&ach.id.0) {
                        let _ = unlock_achievement(
                            db,
                            user_id,
                            &ach.id.0,
                            ach.xp_reward,
                            ach.crystal_reward,
                        )
                        .await;

                        let notif = Notification::new(
                            user_id,
                            NotificationType::AchievementUnlocked,
                            ach.name.clone(),
                            format!("Achievement Unlocked: {}", ach.description),
                        );
                        let _ = insert_notification(db, &notif).await;
                    }
                }
            }

            // Level-based achievements
            let level_unlocked =
                tracker.check_unlocks("_current", "player_level", profile.level as u32);
            for ach in level_unlocked {
                if !unlocked_ids.contains(&ach.id.0) {
                    let _ = unlock_achievement(
                        db,
                        user_id,
                        &ach.id.0,
                        ach.xp_reward,
                        ach.crystal_reward,
                    )
                    .await;

                    let notif = Notification::new(
                        user_id,
                        NotificationType::AchievementUnlocked,
                        ach.name.clone(),
                        format!("Achievement Unlocked: {}", ach.description),
                    );
                    let _ = insert_notification(db, &notif).await;
                }
            }

            // Lifetime XP milestone (million)
            if profile.total_xp_earned >= 1_000_000 {
                let xp_unlocked = tracker.check_unlocks(
                    "_current",
                    "lifetime_xp_millions",
                    (profile.total_xp_earned / 1_000_000) as u32,
                );
                for ach in xp_unlocked {
                    if !unlocked_ids.contains(&ach.id.0) {
                        let _ = unlock_achievement(
                            db,
                            user_id,
                            &ach.id.0,
                            ach.xp_reward,
                            ach.crystal_reward,
                        )
                        .await;

                        let notif = Notification::new(
                            user_id,
                            NotificationType::AchievementUnlocked,
                            ach.name.clone(),
                            format!("Achievement Unlocked: {}", ach.description),
                        );
                        let _ = insert_notification(db, &notif).await;
                    }
                }
            }
        }
    }

    match event_type {
        // ── Task lifecycle ───────────────────────────────
        "task_completed" => {
            companion.interact(Interaction::TaskCompleted);
            companion.code_quality = (companion.code_quality + 1).min(100);
            companion_changed = true;
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Improve).await;
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::AgentComplete,
            )
            .await;
            profile_changed = true;
        }
        "bug_fix" | "bug_battle_won" => {
            companion.interact(Interaction::TaskCompleted);
            companion_changed = true;
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Battle).await;
            profile_changed = true;
        }
        "task_started" | "task_submitted" => {
            companion.interact(Interaction::TaskAssigned);
            companion_changed = true;
        }
        "lock_acquired" => {
            companion.interact(Interaction::LockAcquired);
            companion_changed = true;
        }
        "lock_released" => {
            companion.interact(Interaction::Rest);
            companion_changed = true;
        }
        "snapshot_captured" => {
            companion.code_quality = (companion.code_quality + 1).min(100);
            companion_changed = true;
        }
        "task_failed" => {
            companion.interact(Interaction::TaskFailed);
            companion_changed = true;
        }

        // ── Collaboration ────────────────────────────────
        "plan_handoff" | "agent_handoff_accepted" | "peer_teach_session" => {
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::Collaborate,
            )
            .await;
            profile_changed = true;
        }

        // ── Code Quality ─────────────────────────────────
        "refactor" | "fmt_applied" | "toestub_violations_fixed" => {
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Improve).await;
            if event_type == "toestub_violations_fixed" {
                advance_quests(
                    db,
                    &mut profile,
                    user_id,
                    crate::quest::QuestType::ToestubFix,
                )
                .await;
            }
            profile_changed = true;
        }
        "test_pass" | "test_coverage_improved" | "test_suite_green" => {
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Testing).await;
            profile_changed = true;
        }
        "doc_added" | "doc_coverage_100_pct" | "missing_docs_zero" => {
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::DocSprint,
            )
            .await;
            profile_changed = true;
        }

        // ── AI & Mens ──────────────────────────────────
        "ai_thumbs_up" | "ai_thumbs_down" => {
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::AiFeedback,
            )
            .await;
            profile_changed = true;
        }
        "populi_corpus_contributed" => {
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::PopuliContribute,
            )
            .await;
            profile_changed = true;
        }

        // ── Package & Registry ──────────────────────────
        "pkg_published" | "mcp_tool_registered" => {
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Create).await;
            profile_changed = true;
        }

        // ── Cost & Security ──────────────────────────────
        "cost_incurred" => {
            profile.spend_energy(1);
            profile_changed = true;
            companion_changed = true;
        }
        "unsafe_removed" => {
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Improve).await;
            profile_changed = true;
        }
        "security_review_passed" => {
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Improve).await;
            advance_quests(db, &mut profile, user_id, crate::quest::QuestType::Review).await;
            profile_changed = true;
        }
        "activity_changed" => {
            if let Some(act) = event_kind.get("activity").and_then(|v| v.as_str()) {
                let interaction = match act {
                    "writing" => Interaction::Writing,
                    "idle" => Interaction::Idle,
                    _ => Interaction::Idle,
                };
                companion.interact(interaction);
                companion_changed = true;
            }
        }
        _ => {}
    }

    // 4. Persist changes
    if profile_changed {
        upsert_profile(db, &profile).await?;
    }
    if companion_changed && agent_id_str.is_some() {
        upsert_companion(db, &companion).await?;
    }

    Ok(crate::reward_policy::RouteResult {
        reward: final_reward,
        leveled_up: leveled_up_info.map(|(lvl, title, _xp)| (lvl, title)),
    })
}
