//! Quests, leaderboard, notifications, hints, achievements.

use crate::commands::extras::ludus::LudusContext;

use anyhow::Result;
use owo_colors::OwoColorize;
use vox_gamify::{db, quest};

use super::db_util;
use super::progress::render_progress_bar;

/// List daily quests.
pub async fn quest_list() -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;

    // Generate deterministic daily quests based on the day
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let day_seed = now / 86_400;

    let user_id = user_id.clone();
    let mut quests = db::list_quests(db, &user_id).await?;

    // Filter out expired quests or if none exist for today, generate new ones
    if quests.is_empty() || quests.iter().all(|q| q.is_expired()) {
        quests = quest::generate_daily_quests(&user_id, day_seed);
        for q in &quests {
            db::upsert_quest(db, q).await?;
        }
    }

    println!("{}", "╔══════════════════════════════════╗".bright_yellow());
    println!("{}", "║       📋 Daily Quests           ║".bright_yellow());
    println!("{}", "╚══════════════════════════════════╝".bright_yellow());
    println!();

    for q in &quests {
        let progress_bar = render_progress_bar(q.progress_pct(), 20);
        let status_icon = if q.completed { "✅" } else { "⬜" };

        println!(
            "  {} {} {}",
            status_icon,
            q.quest_type.emoji(),
            q.description.bright_white(),
        );
        println!(
            "    {} {}/{}  💎{} ⭐{}",
            progress_bar,
            q.progress,
            q.target,
            q.crystal_reward.to_string().bright_yellow(),
            q.xp_reward.to_string().bright_cyan(),
        );
        println!("    💡 {}", q.hint.dimmed());
        println!();
    }

    Ok(())
}

/// Generate quests: workspace scanner (TODOs/FIXMEs) + daily archetype templates.
pub async fn quest_generate() -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;

    println!("{}", "╔══════════════════════════════════╗".bright_yellow());
    println!("{}", "║   📋 Generating Quests...       ║".bright_yellow());
    println!("{}", "╚══════════════════════════════════╝".bright_yellow());
    println!();

    // 1. Workspace-scan dynamic quests (from TODOs/FIXMEs)
    let workspace_root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let existing_quests = db::list_quests(db, user_id).await.unwrap_or_default();
    let active_count = existing_quests
        .iter()
        .filter(|q| !q.completed && !q.is_expired())
        .count();

    let dynamic = vox_gamify::quest_engine::generate_dynamic_quests(
        user_id,
        &workspace_root,
        active_count,
        5,
    );

    for dq in &dynamic {
        db::upsert_quest(db, &dq.quest).await?;
        let src = dq
            .source_issue
            .as_ref()
            .map(|i| format!(" ({}:{})", i.file_path.display(), i.line))
            .unwrap_or_default();
        println!(
            "  ⚔️  {} {}{}",
            dq.quest.quest_type.emoji(),
            dq.quest.description.bright_white(),
            src.dimmed(),
        );
        if let Some(hint) = &dq.hint {
            println!("     💡 {}", hint.dimmed());
        }
    }

    // 2. Daily archetype fallback if no dynamic quests
    if dynamic.is_empty() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let day_seed = now / 86_400;
        let quests = quest::generate_daily_quests(user_id, day_seed);
        for q in &quests {
            db::upsert_quest(db, q).await?;
        }
        println!("  (No workspace issues found — using daily archetype quests)");
    }

    println!(
        "  Use {} to view and track them.",
        "vox ludus quest list".bright_green()
    );

    Ok(())
}

/// Show the user leaderboard.
pub async fn leaderboard_show(metric: &str, limit: usize) -> Result<()> {
    let db = db_util::get_db().await?;
    let metric_lower = metric.to_lowercase();
    let entries = if metric_lower == "lumens" || metric_lower == "karma" {
        db::lumens_leaderboard(&db, limit as i64).await?
    } else {
        db::leaderboard(&db, limit as i64).await?
    };

    println!("{}", "╔══════════════════════════════════╗".bright_yellow());
    println!("║       🏆 {} Leaderboard       ║", metric.to_uppercase());
    println!("{}", "╚══════════════════════════════════╝".bright_yellow());
    println!();

    if entries.is_empty() {
        println!("  No entries found.");
    } else {
        println!(
            "  {:>3} {:<20} {:>10}",
            "Rk",
            "User".bright_white(),
            metric.to_uppercase().bright_white()
        );
        println!("  {}", "─".repeat(35).dimmed());

        for (i, entry) in entries.iter().enumerate() {
            let rank = i + 1;
            let medal = match rank {
                1 => "🥇".to_string(),
                2 => "🥈".to_string(),
                3 => "🥉".to_string(),
                _ => rank.to_string(),
            };
            println!(
                "  {:>3} {:<20} {:>10}",
                medal,
                entry.user_id.bright_white().bold(),
                entry.score.to_string().bright_cyan()
            );
        }
    }

    Ok(())
}

/// List pending notifications. When `mark_read`, marks all listed rows read after printing.
pub async fn notify_list(mark_read: bool) -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    let notifications = db::list_unread_notifications(db, user_id, 10).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║       🔔 Notifications          ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();

    if notifications.is_empty() {
        println!("  You have no pending notifications.");
    } else {
        for n in notifications {
            let icon = match n.notification_type {
                vox_gamify::notifications::NotificationType::LevelUp => "🆙",
                vox_gamify::notifications::NotificationType::AchievementUnlocked => "🏆",
                vox_gamify::notifications::NotificationType::QuestCompleted => "📋",
                vox_gamify::notifications::NotificationType::ChallengeCompleted => "⚔️",
                vox_gamify::notifications::NotificationType::BattleWon => "🎉",
                vox_gamify::notifications::NotificationType::BattleLost => "💔",
                vox_gamify::notifications::NotificationType::ArenaJoined => "🏟️",
                vox_gamify::notifications::NotificationType::CollegiumJoined => "🏫",
                vox_gamify::notifications::NotificationType::CompanionCreated => "🐣",
                vox_gamify::notifications::NotificationType::ItemPurchased => "🛍️",
                vox_gamify::notifications::NotificationType::FeedbackReceived => "💬",
                _ => "ℹ️",
            };
            println!(
                "  {} {}  {}",
                icon,
                vox_gamify::util::format_unix_time(n.created_at).dimmed(),
                if n.read {
                    n.message.dimmed().to_string()
                } else {
                    n.message.bright_white().to_string()
                }
            );
        }
        if mark_read {
            db::mark_all_notifications_read(db, user_id).await?;
        }
    }

    Ok(())
}

/// Clear all notifications.
pub async fn notify_clear() -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    db::mark_all_notifications_read(db, user_id).await?;
    println!("  ✅ Notifications cleared.");
    Ok(())
}

/// Show a contextual hint.
pub async fn hint_show(context: Option<&str>) -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    let mut profile = vox_gamify::db::get_teaching_profile(db, user_id).await?;
    let freq = vox_gamify::config_gate::mode().hint_frequency();
    let kind = match context {
        Some("build") => vox_gamify::teaching::MistakeKind::ArchitecturalIssue,
        Some("test") | Some("tests") => vox_gamify::teaching::MistakeKind::TestFailure,
        Some("battle") => {
            println!(
                "  💡 {} Companions with higher 'Code Quality' deal more damage in battles.",
                "Pro Tip:".bright_yellow().bold()
            );
            return Ok(());
        }
        _ => vox_gamify::teaching::MistakeKind::TodoDebt,
    };
    let req = profile.record_mistake(kind, freq);
    vox_gamify::db::upsert_teaching_profile(db, &profile).await?;

    let hint = if let Some(ref r) = req {
        let _ = vox_gamify::db::log_hint_event(
            db,
            user_id,
            &format!("{:?}", r.kind),
            "pull_hint",
            context,
        )
        .await;
        vox_gamify::teaching::Hint::deterministic(r).body
    } else {
        let _ = vox_gamify::db::log_hint_event(
            db,
            user_id,
            &format!("{kind:?}"),
            "suppressed",
            context,
        )
        .await;
        match context {
            Some("build") => {
                "Try using `vox check` before `vox build` to catch errors faster.".to_string()
            }
            _ => "You can adopt multiple companions, but only one can join you in a battle."
                .to_string(),
        }
    };

    println!("  💡 {} {}", "Pro Tip:".bright_yellow().bold(), hint);
    Ok(())
}

/// List all glyphs and achievements.
pub async fn glyph_list(unlocked_only: bool) -> Result<()> {
    let ctx = LudusContext::load().await?;
    let db = &ctx.db;
    let user_id = &ctx.user_id;
    let tracker = vox_gamify::achievement::AchievementTracker::new();
    let unlocked = db::list_unlocked_achievements(db, user_id).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_white());
    println!("{}", "║       ⭐ Glyphs & Achievements  ║".bright_white());
    println!("{}", "╚══════════════════════════════════╝".bright_white());
    println!();

    for ach in tracker.all_achievements() {
        let is_unlocked = unlocked.iter().any(|(id, _)| id == &ach.id.0);
        if unlocked_only && !is_unlocked {
            continue;
        }

        let icon = if is_unlocked { "✨" } else { "🔒" };
        let name = if is_unlocked {
            ach.name.bright_white().bold().to_string()
        } else {
            ach.name.dimmed().to_string()
        };

        println!(
            "  {} {} - {}",
            icon,
            name,
            ach.description.italic().dimmed()
        );
        if is_unlocked {
            println!(
                "     ⭐ {}  💎 {}",
                ach.xp_reward.to_string().bright_cyan(),
                ach.crystal_reward.to_string().bright_yellow()
            );
        }
        println!();
    }

    Ok(())
}
