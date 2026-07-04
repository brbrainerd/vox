use anyhow::Result;
use owo_colors::OwoColorize;
use vox_gamify::db as ludus_db;

use crate::commands::extras::ludus::{db_util, render_progress_bar};

/// Show current arena event status.
pub async fn arena_show() -> Result<()> {
    let ctx = crate::commands::extras::ludus::LudusContext::load().await?;
    let codex = &ctx.db;
    let user_id = &ctx.user_id;

    let event = ludus_db::get_active_arena_event(codex).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║       🏟️  THE ARENA EVENT       ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();

    if let Some(ev) = event {
        let progress_pct = (ev.current_xp as f64 / ev.target_xp.max(1) as f64).min(1.0);

        println!("  Event: {}", ev.name.bright_white().bold());
        println!("  {}", ev.description.italic().dimmed());
        println!();

        let progress_bar = render_progress_bar(progress_pct, 25);
        println!(
            "  Community Progress: {}  {} / {}",
            progress_bar,
            ev.current_xp.to_string().bright_cyan(),
            ev.target_xp.to_string().bright_white()
        );
        println!();

        let (my_xp, my_lumens) = ludus_db::get_arena_contribution(codex, &ev.id, user_id).await?;
        println!(
            "  Your Contribution:  ⭐ {} XP  ✦ {} Lumens",
            my_xp.to_string().bright_cyan(),
            my_lumens.to_string().bright_yellow()
        );
    } else {
        println!("  No active arena events at this time.");
        println!("  Check back later for new community challenges!");
    }

    Ok(())
}

/// Join the current arena event.
pub async fn arena_join() -> Result<()> {
    let ctx = crate::commands::extras::ludus::LudusContext::load().await?;
    let codex = &ctx.db;
    let user_id = &ctx.user_id;

    if let Some(ev) = ludus_db::get_active_arena_event(codex).await? {
        ludus_db::join_arena_event(codex, &ev.id, user_id).await?;
        println!(
            "{}",
            format!("🏟️  Arena Event Joined! Ready for quest: {}", ev.name)
                .bright_green()
                .bold()
        );

        let event_json = serde_json::json!({
            "type": "arena_joined",
            "arena_id": ev.id,
        });
        let res = vox_gamify::event_router::route_event(codex, user_id, &event_json).await?;
        crate::commands::extras::ludus::print_route_result(&res);
    } else {
        println!("  ❌ No active arena events to join.");
    }
    Ok(())
}

/// Show the arena leaderboard.
pub async fn arena_leaderboard() -> Result<()> {
    let codex = db_util::get_db().await?;

    if let Some(ev) = ludus_db::get_active_arena_event(&codex).await? {
        let entries = ludus_db::arena_event_leaderboard(&codex, &ev.id, 10).await?;

        println!("{}", "╔══════════════════════════════════╗".bright_cyan());
        println!("{}", "║       🏟️  ARENA LEADERBOARD     ║".bright_cyan());
        println!("{}", "╚══════════════════════════════════╝".bright_cyan());
        println!();

        println!("  Rank  User                  Contrib (XP/Lumens)");
        println!("  {}", "─".repeat(50).dimmed());

        for (i, (uid, xp, lumens)) in entries.into_iter().enumerate() {
            let medal = match i + 1 {
                1 => "🥇".to_string(),
                2 => "🥈".to_string(),
                3 => "🥉".to_string(),
                r => r.to_string(),
            };
            println!(
                "  {:>3} {:<20} ⭐ {} / ✦ {}",
                medal,
                uid.bright_white(),
                xp.to_string().bright_cyan(),
                lumens.to_string().bright_yellow()
            );
        }
    } else {
        println!("  No active arena events.");
    }

    Ok(())
}
