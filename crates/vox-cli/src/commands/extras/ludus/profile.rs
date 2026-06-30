//! Profile and activity commands.

use anyhow::Result;
use owo_colors::OwoColorize;
use vox_config::GamifyMode;
use vox_gamify::{db, profile::LudusProfile};

use vox_bounded_fs::read_utf8_path_capped;

use super::db_util;

/// Show the daily gamification digest.
pub async fn morning_digest() -> Result<()> {
    if !vox_gamify::config_gate::is_enabled() {
        return Ok(());
    }
    if matches!(
        vox_gamify::config_gate::ludus_channel(),
        vox_gamify::config_gate::LudusChannel::DigestPriority
    ) {
        // Prefer `vox ludus digest-weekly` for digest-first users.
        return Ok(());
    }
    let db = match db_util::get_db().await {
        Ok(db) => db,
        Err(_) => return Ok(()),
    };
    let user_id = vox_gamify::db::canonical_user_id();

    let mut profile = db::get_profile(&db, &user_id)
        .await
        .unwrap_or_default()
        .unwrap_or_else(|| LudusProfile::new_default(&user_id));
    profile.regen_energy();
    let _ = db::upsert_profile(&db, &profile).await;

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║      🌅 Vox Ludus Morning      ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();

    println!(
        "  Good morning, {}!",
        profile.title().bright_yellow().bold()
    );
    println!(
        "  Streak: {} 🔥  |  Level: {}  |  Crystals: {} 💎",
        profile.streak.current_streak.to_string().bright_yellow(),
        profile.level.to_string().bright_cyan(),
        profile.crystals.to_string().bright_cyan()
    );
    println!();

    // Quests
    let quests = db::list_quests(&db, &user_id).await.unwrap_or_default();
    println!("  {}", "📋 Active Quests for Today:".bold());
    if quests.is_empty() {
        println!("    No quests generated yet. Do some coding or run `vox ludus quest-generate`.");
    } else {
        let active = quests.iter().filter(|q| !q.completed).count();
        if active == 0 {
            println!("    All quests completed! Great job.");
        }
        for q in quests.iter().filter(|q| !q.completed) {
            println!(
                "    {}: {}",
                q.quest_type.emoji(),
                q.description.bright_white()
            );
            println!(
                "      Reward: {} XP  |  {} 💎",
                q.xp_reward.to_string().bright_yellow(),
                q.crystal_reward.to_string().bright_cyan()
            );
        }
    }
    println!();

    // Companion
    if let Ok(companions) = db::list_companions(&db, &user_id).await {
        if let Some(c) = companions.first() {
            println!(
                "  {} {} is feeling {}",
                c.mood.emoji(),
                c.name.bright_green(),
                format!("{:?}", c.mood).bright_yellow()
            );
            println!();
        }
    }

    Ok(())
}

/// Record a daily activity action and display a subtle message if a streak/level changes.
pub async fn record_activity() -> Result<()> {
    let db = match db_util::get_db().await {
        Ok(db) => db,
        Err(_) => return Ok(()),
    };

    let user_id = vox_gamify::db::canonical_user_id();
    let mut profile = match db::get_profile(&db, &user_id).await.unwrap_or(None) {
        Some(p) => p,
        None => {
            let p = LudusProfile::new_default(&user_id);
            let _ = db::upsert_profile(&db, &p).await;
            p
        }
    };

    let old_level = profile.level;
    let result = profile.record_daily_activity();

    let _ = db::upsert_profile(&db, &profile).await;

    use vox_gamify::streak::StreakResult;
    if vox_gamify::output_policy::should_emit_cli_celebration() {
        match result {
            StreakResult::Continued { streak, bonus_xp } if streak > 1 => {
                println!(
                    "  🔥 {} {} ({} XP)",
                    "Streak continued:".bright_yellow(),
                    streak,
                    bonus_xp.to_string().bright_cyan()
                );
            }
            StreakResult::SavedByGrace { streak, bonus_xp } => {
                println!(
                    "  🛡️ {} {} ({} XP)",
                    "Streak saved by grace period:".bright_green(),
                    streak,
                    bonus_xp.to_string().bright_cyan()
                );
            }
            StreakResult::BrokenReset { previous } if previous > 1 => {
                println!(
                    "  🌱 {} (previous run: {} days — new streak starts now)",
                    "Streak paused".bright_yellow(),
                    previous
                );
            }
            _ => {}
        }

        if profile.level > old_level {
            println!(
                "  🏆 {} You reached {}!",
                "LEVEL UP!".bright_magenta().bold(),
                format!("Level {}", profile.level).bright_yellow()
            );
        }
    }

    Ok(())
}

/// Fire-and-forget passive ludus event recording after a CLI command completes.
///
/// Spawns a background tokio task so it never blocks the main CLI flow.
/// Records: daily activity streak + the specific event type (e.g., "build_completed").
/// When `capability_id` and `command_path` are provided, also logs to Codex `cli_command_events`
/// for unified capability telemetry.
pub fn record_cli_event_fire_and_forget(
    event_type: &'static str,
    success: bool,
    capability_id: Option<&'static str>,
    command_path: Option<&'static str>,
) {
    tokio::spawn(async move {
        let _ = record_cli_event_inner(event_type, success, capability_id, command_path).await;
    });
}

async fn record_cli_event_inner(
    event_type: &str,
    success: bool,
    capability_id: Option<&str>,
    command_path: Option<&str>,
) -> anyhow::Result<()> {
    let db = match db_util::get_db().await {
        Ok(db) => db,
        Err(_) => return Ok(()),
    };

    let user_id = vox_db::paths::local_user_id();

    // Codex: log CLI command event for unified capability telemetry (runs regardless of ludus)
    if let (Some(cap_id), Some(cmd_path)) = (capability_id, command_path) {
        let metadata = serde_json::json!({
            "capability_id": cap_id,
            "success": success,
        })
        .to_string();
        let _ = db
            .record_behavior_event(&user_id, "cli_command", Some(cmd_path), Some(&metadata))
            .await;
    }

    if !vox_gamify::config_gate::is_enabled() {
        return Ok(());
    }

    let ludus_uid = vox_gamify::db::canonical_user_id();

    // Single authority: `process_event_rewards` (via route_event) performs daily activity + profile XP.
    let event_json = serde_json::json!({
        "type": event_type,
        "success": success,
        "agent_id": 0u64,
    });
    let _ = vox_gamify::event_router::route_event(&db, &ludus_uid, &event_json).await;

    Ok(())
}

/// Display gamification status (profile overview).
pub async fn status() -> Result<()> {
    let ctx = crate::commands::extras::ludus::LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    let profile = &ctx.profile;

    println!("{}", "╔══════════════════════════════════╗".bright_purple());
    println!("{}", "║        ⚡ Vox Ludus ⚡          ║".bright_purple());
    println!("{}", "╚══════════════════════════════════╝".bright_purple());
    println!();

    if profile.reward_suppressed {
        println!(
            "{}",
            "███████████████████████████████████████████████████████████".red()
        );
        println!(
            "{}",
            "█ ⚠️  WARNING: LUDUS REWARDS SUPPRESSED DUE TO PENALTY ⚠️  █"
                .red()
                .bold()
        );
        println!(
            "{}",
            "███████████████████████████████████████████████████████████".red()
        );
        println!();
    }

    // Identity link display — the GitHub login lives in Clavis (github.com
    // registry username), set during `vox ludus auth`.
    if let Some(login) = vox_secrets::get_registry_username("github.com") {
        if !login.is_empty() {
            println!("  🔗 Linked GitHub: {}", login.bright_blue());
        }
    }
    println!(
        "  {} Trust Tier: {}",
        profile.trust_tier.icon(),
        profile.trust_tier.label().bright_white().bold()
    );
    println!();

    let title = vox_gamify::full_title(profile.level, profile.prestige_level);
    println!(
        "  🏅 {}       Level {}  •  {} XP to next level",
        title.bright_yellow().bold(),
        profile.level.to_string().bright_yellow(),
        profile.xp_to_next_level().to_string().bright_cyan(),
    );
    println!(
        "  💎 {} crystals  •  ⚡ {}/{} energy  •  ✦ {} lumens",
        profile.crystals.to_string().bright_yellow(),
        profile.energy.to_string().bright_green(),
        profile.max_energy,
        profile.lumens.to_string().bright_yellow(),
    );
    if profile.streak_shields > 0 {
        println!(
            "  🛡️  Streak Shields: {}",
            profile.streak_shields.to_string().bright_green()
        );
    }
    println!();

    let client = vox_gamify::FreeAiClient::auto_discover().await;
    println!("  🤖 AI providers:");
    for provider in client.providers() {
        println!("    • {}", provider.name().bright_blue());
    }
    println!();

    let tasks_today = db::get_counter(db, user_id, "tasks_today")
        .await
        .unwrap_or(0);
    println!(
        "  📈 Activity Today: {} tasks completed",
        tasks_today.to_string().bright_cyan()
    );

    let achievements = db::list_unlocked_achievements(db, user_id)
        .await
        .unwrap_or_default();
    if !achievements.is_empty() {
        println!(
            "  🎖️ Achievements Unlocked: {}",
            achievements.len().to_string().bright_yellow()
        );
        // Show up to 5 most recent
        for (id, earned_at) in achievements.into_iter().take(5) {
            // we'd prefer to get the real name, but ID works for a quick display
            println!(
                "    • {} (on {})",
                id.bright_white(),
                earned_at.bright_black()
            );
        }
        println!();
    }

    println!(
        "  Use {} to record daily activity",
        "vox ludus record".bright_green()
    );
    println!();

    Ok(())
}

/// Rate an AI response and earn XP.
pub async fn feedback_rate(
    session_id: &str,
    response_id: &str,
    thumbs_up: bool,
    comment: Option<&str>,
    example: Option<&std::path::Path>,
) -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();

    let mut feedback = vox_gamify::feedback::AiFeedback::new(
        uuid::Uuid::new_v4().to_string(),
        &user_id,
        session_id,
        response_id,
        thumbs_up,
        vox_gamify::util::now_unix(),
    );

    if let Some(c) = comment {
        feedback = feedback.with_comment(c);
    }

    if let Some(ex_path) = example {
        let code = read_utf8_path_capped(ex_path)
            .map_err(|e| anyhow::anyhow!("Failed to read example file: {}", e))?;
        feedback = feedback.with_example(code);
    }

    let should_contribute = vox_gamify::feedback::should_auto_contribute(&feedback);
    if should_contribute {
        feedback = feedback.mark_corpus_contributed();
    }

    vox_gamify::db::insert_feedback(&db, &feedback).await?;

    let event_type = if thumbs_up {
        "ai_thumbs_up"
    } else {
        "ai_thumbs_down"
    };
    let event_json = serde_json::json!({
        "type": event_type,
        "success": true,
        "agent_id": 0u64,
        "feedback_id": feedback.id,
    });

    println!(
        "  ✅ Feedback recorded for response {}",
        response_id.bright_cyan()
    );
    if should_contribute {
        println!("  🌟 Valid example provided! Feedback forwarded to Mens training corpus.");

        let corpus_event = serde_json::json!({
            "type": "populi_corpus_contributed",
            "success": true,
            "agent_id": 0u64,
            "feedback_id": feedback.id,
        });
        let _ = vox_gamify::event_router::route_event(&db, &user_id, &corpus_event).await;
    }

    let _ = vox_gamify::event_router::route_event(&db, &user_id, &event_json).await;

    Ok(())
}

/// Claim available daily or weekly periodic rewards.
pub async fn reward_claim() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();

    let weekly_reward = vox_gamify::periodic_reward::current_weekly_reward(&user_id);

    let existing = vox_gamify::db::get_reward_claim(&db, &user_id, &weekly_reward.id).await?;
    if let Some(mut claim) = existing {
        if claim.redeemed {
            println!(
                "  ℹ️ Weekly reward '{}' already claimed.",
                claim.name.bright_yellow()
            );
            return Ok(());
        }
        claim.claim();
        vox_gamify::db::upsert_periodic_reward(&db, &claim, &user_id).await?;

        let event = serde_json::json!({
            "type": "periodic_reward_claimed",
            "success": true,
            "agent_id": 0u64,
            "reward_id": claim.id,
            "xp_bonus": claim.xp_bonus,
            "crystal_bonus": claim.crystal_bonus,
        });
        let _ = vox_gamify::event_router::route_event(&db, &user_id, &event).await;

        println!(
            "  🎁 Claimed weekly reward: {}",
            claim.name.bright_white().bold()
        );
        println!(
            "  💎 +{} crystals  ⭐ +{} XP",
            claim.crystal_bonus.to_string().bright_yellow(),
            claim.xp_bonus.to_string().bright_cyan()
        );
    } else {
        let eligible = vox_gamify::periodic_reward::evaluate_condition(
            &db,
            &user_id,
            &weekly_reward.unlock_condition,
        )
        .await;
        if eligible {
            let mut claim = weekly_reward.clone();
            claim.claim();
            vox_gamify::db::upsert_periodic_reward(&db, &claim, &user_id).await?;

            let event = serde_json::json!({
                "type": "periodic_reward_claimed",
                "success": true,
                "agent_id": 0u64,
                "reward_id": claim.id,
                "xp_bonus": claim.xp_bonus,
                "crystal_bonus": claim.crystal_bonus,
            });
            let _ = vox_gamify::event_router::route_event(&db, &user_id, &event).await;

            println!(
                "  🎁 Claimed weekly reward: {}",
                claim.name.bright_white().bold()
            );
            println!(
                "  💎 +{} crystals  ⭐ +{} XP",
                claim.crystal_bonus.to_string().bright_yellow(),
                claim.xp_bonus.to_string().bright_cyan()
            );
        } else {
            println!(
                "  🔒 Weekly reward '{}' is not yet eligible to be claimed.",
                weekly_reward.name.bright_yellow()
            );
            println!("  Condition: {}", weekly_reward.description.dimmed());
        }
    }

    Ok(())
}

/// View or change the gamification mode (`effective`: include session env overlay).
pub async fn mode_command(set: Option<&str>, effective: bool) -> Result<()> {
    let mut cfg = vox_gamify::config_gate::load_disk();

    if let Some(mode_str) = set {
        match mode_str.to_lowercase().as_str() {
            "off" => {
                cfg.gamify_enabled = false;
                cfg.save()
                    .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;
                println!("  ✅ Gamification mode set to: {}", mode_str.bright_green());
            }
            "balanced" | "serious" | "learning" => {
                cfg.gamify_enabled = true;
                cfg.gamify_mode = match mode_str.to_lowercase().as_str() {
                    "serious" => GamifyMode::Serious,
                    "learning" => GamifyMode::Learning,
                    _ => GamifyMode::Balanced,
                };
                cfg.save()
                    .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;
                println!("  ✅ Gamification mode set to: {}", mode_str.bright_green());
            }
            _ => {
                println!(
                    "  ❌ Unknown mode '{}'. Use: balanced, serious, learning, off",
                    mode_str.bright_red()
                );
            }
        };
    } else {
        let status = if cfg.gamify_enabled {
            cfg.gamify_mode.as_config_str().bright_green().to_string()
        } else {
            "off".bright_red().to_string()
        };
        println!("  On-disk mode: {}", status);
        if effective {
            let eff = vox_gamify::config_gate::load_effective();
            let eff_s = if eff.gamify_enabled {
                eff.gamify_mode.as_config_str().bright_cyan().to_string()
            } else {
                "off".bright_red().to_string()
            };
            println!("  Effective mode (env/session): {}", eff_s);
            println!(
                "  {}",
                "Session overrides: VOX_LUDUS_SESSION_ENABLED, VOX_LUDUS_SESSION_MODE".dimmed()
            );
            println!(
                "  {}",
                "Emergency kill-switch: VOX_LUDUS_EMERGENCY_OFF=1".dimmed()
            );
        }
        println!(
            "  Change with: {}",
            "vox ludus mode --set <balanced|serious|learning|off>".dimmed()
        );
    }

    Ok(())
}

/// Persist-enable Ludus (keeps current mode).
pub async fn enable_ludus() -> Result<()> {
    let mut cfg = vox_gamify::config_gate::load_disk();
    cfg.gamify_enabled = true;
    cfg.save()
        .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;
    println!("  ✅ {}", "Ludus enabled (saved to config).".bright_green());
    Ok(())
}

/// Persist-disable Ludus.
pub async fn disable_ludus() -> Result<()> {
    let mut cfg = vox_gamify::config_gate::load_disk();
    cfg.gamify_enabled = false;
    cfg.save()
        .map_err(|e| anyhow::anyhow!("Failed to save config: {}", e))?;
    println!(
        "  ✅ {}",
        "Ludus disabled (saved to config).".bright_green()
    );
    Ok(())
}

/// Merge synthetic `default` profile into the current local user when local is empty.
pub async fn profile_merge_from_default() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_db::paths::local_user_id();
    if db::merge_default_profile_into_user(&db, &user_id).await? {
        println!(
            "  ✅ Merged Ludus progress from `default` into `{}`.",
            user_id.bright_green()
        );
    } else {
        println!("  ℹ️  No merge needed (local profile exists or `default` is empty).");
    }
    Ok(())
}

/// Recent reward-policy snapshots (how Ludus interpreted recent events).
pub async fn audit_show(limit: usize) -> Result<()> {
    let ctx = crate::commands::extras::ludus::LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    let rows = db::list_recent_policy_snapshots(db, user_id, limit).await?;
    println!(
        "{}",
        "Ludus policy audit (recent awards)".bright_cyan().bold()
    );
    if rows.is_empty() {
        println!("  (no policy snapshots yet — enable Ludus and generate events.)");
        return Ok(());
    }
    for r in rows {
        let cap = if r.grind_capped != 0 { " capped" } else { "" };
        println!(
            "  {:<22} {:>4} XP / {:>3} 💎  ×{:.2} [{}]{}  {}  {}",
            r.event_type.bright_white(),
            r.awarded_xp,
            r.awarded_crystals,
            r.effective_multiplier,
            r.mode_label.dimmed(),
            cap.dimmed(),
            r.created_at.dimmed(),
            r.metadata.as_deref().unwrap_or("").dimmed()
        );
    }
    Ok(())
}

/// Local KPI summary from policy snapshots + hint telemetry.
pub async fn metrics_show() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let k = db::load_kpi_summary(&db, &user_id).await?;
    println!("{}", "Ludus metrics (local user)".bright_cyan().bold());
    println!("  Events (policy rows): {}", k.events_recorded);
    println!("  Total XP awarded: {}", k.total_xp_awarded);
    println!("  Total crystals: {}", k.total_crystals_awarded);
    println!("  Grind-capped events: {}", k.grind_capped_events);
    println!(
        "  Avg effective multiplier: {:.2}",
        k.avg_effective_multiplier
    );
    println!("  Hint telemetry rows: {}", k.hint_events_logged);
    Ok(())
}

/// Short post-session digest: profile + KPI headliners.
pub async fn session_digest() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let k = db::load_kpi_summary(&db, &user_id).await?;
    let profile = db::get_profile(&db, &user_id)
        .await?
        .unwrap_or_else(|| LudusProfile::new_default(&user_id));
    println!("{}", "═══ Ludus session digest ═══".bright_cyan());
    println!(
        "  {}  streak {}  💎{}  ✦{}",
        profile.title().bright_yellow(),
        profile.streak.current_streak,
        profile.crystals,
        profile.lumens
    );
    println!(
        "  All-time policy events: {} ({} grind-capped)",
        k.events_recorded, k.grind_capped_events
    );
    Ok(())
}

/// Rolling 7-day digest: KPI, unread notifications, recent policy awards.
pub async fn digest_weekly() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let k = db::load_kpi_summary(&db, &user_id).await?;
    let policy = db::list_policy_snapshots_since_days(&db, &user_id, 7, 48).await?;
    let notes = db::list_unread_notifications(&db, &user_id, 20).await?;
    println!("{}", "═══ Ludus 7-day digest ═══".bright_cyan().bold());
    println!(
        "  Policy rows (all-time): {}  |  grind-capped: {}  |  hints: {}",
        k.events_recorded, k.grind_capped_events, k.hint_events_logged
    );
    println!(
        "  Unread notifications: {}  |  policy events (7d window, capped): {}",
        notes.len(),
        policy.len()
    );
    if !notes.is_empty() {
        println!("{}", "  Latest notifications:".dimmed());
        for n in notes.iter().take(8) {
            println!("    • {} — {}", n.title.bright_white(), n.message.dimmed());
        }
    }
    if !policy.is_empty() {
        println!("{}", "  Recent awards (7d):".dimmed());
        for r in policy.iter().take(12) {
            println!(
                "    • {:<20} +{} XP  {}",
                r.event_type,
                r.awarded_xp,
                r.created_at.dimmed()
            );
        }
    }
    Ok(())
}

/// Use a streak shield to protect your daily streak.
pub async fn shield_use() -> Result<()> {
    let db = db_util::get_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let mut profile = db::get_profile(&db, &user_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Profile not found"))?;

    if profile.spend_shield() {
        // Mark today as active so streak doesn't break
        profile.touch();
        db::upsert_profile(&db, &profile).await?;
        println!(
            "  🛡️  {} Success! Your streak is protected for 24 hours.",
            "SHIELD ACTIVATED:".bright_green()
        );
    } else {
        println!("  ❌ You have no streak shields remaining.");
    }

    Ok(())
}
