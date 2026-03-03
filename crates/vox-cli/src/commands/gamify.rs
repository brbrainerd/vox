//! `vox gamify` subcommands — profile, companions, quests, battles.

use anyhow::Result;
use owo_colors::OwoColorize;
use turso::params;
use vox_db::VoxDb;
use vox_gamify::{
    battle::Battle, companion::Mood, db, profile::GamifyProfile, quest, sprite, Companion,
    FreeAiClient,
};

/// Get a local database instance.
async fn get_db() -> Result<VoxDb> {
    VoxDb::connect_default()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to local gamification DB: {}", e))
}

/// Record a daily activity action and display a subtle message if a streak/level changes.
pub async fn record_activity() -> Result<()> {
    // Fail silently if DB is not available
    let db = match get_db().await {
        Ok(db) => db,
        Err(_) => return Ok(()),
    };

    let user_id = vox_db::paths::local_user_id();
    let mut profile = match db::get_profile(&db, &user_id).await.unwrap_or(None) {
        Some(p) => p,
        None => {
            let p = GamifyProfile::new_default(&user_id);
            let _ = db::upsert_profile(&db, &p).await;
            p
        }
    };

    let old_level = profile.level;
    let result = profile.record_daily_activity();

    let _ = db::upsert_profile(&db, &profile).await;

    use vox_gamify::streak::StreakResult;
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
                "  💔 {} (was {} days)",
                "Streak broken".bright_red(),
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

    Ok(())
}

/// Display gamification status (profile overview).
pub async fn status() -> Result<()> {
    let db = get_db().await?;
    let user_id = vox_db::paths::local_user_id();
    let mut profile = match db::get_profile(&db, &user_id).await? {
        Some(p) => p,
        None => {
            let p = GamifyProfile::new_default(&user_id);
            db::upsert_profile(&db, &p).await?;
            p
        }
    };

    profile.regen_energy();
    db::upsert_profile(&db, &profile).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_purple());
    println!("{}", "║     ⚡ Vox Gamification ⚡       ║".bright_purple());
    println!("{}", "╚══════════════════════════════════╝".bright_purple());
    println!();
    println!(
        "  🏅 Level {}  •  {} XP to next level",
        profile.level.to_string().bright_yellow(),
        profile.xp_to_next_level().to_string().bright_cyan(),
    );
    println!(
        "  💎 {} crystals  •  ⚡ {}/{} energy",
        profile.crystals.to_string().bright_yellow(),
        profile.energy.to_string().bright_green(),
        profile.max_energy,
    );
    println!();

    // Show AI provider status
    let client = FreeAiClient::auto_discover().await;
    println!("  🤖 AI providers:");
    for provider in client.providers() {
        println!("    • {}", provider.name().bright_blue());
    }
    println!();
    println!(
        "  Use {} to see your companions",
        "vox gamify companion list".bright_green()
    );
    println!(
        "  Use {} to see daily quests",
        "vox gamify quest list".bright_green()
    );

    Ok(())
}

/// List all companions.
pub async fn companion_list() -> Result<()> {
    let db = get_db().await?;
    let user_id = vox_db::paths::local_user_id();
    let companions = db::list_companions(&db, &user_id).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║       🐱 Your Companions        ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();

    if companions.is_empty() {
        println!("  You have no companions yet.");
    } else {
        for companion in companions {
            let sprite_text = companion
                .ascii_sprite
                .clone()
                .unwrap_or_else(|| sprite::generate_deterministic(&companion.name, companion.mood));
            println!(
                "  {} {} {} [{}]",
                companion.mood.emoji(),
                companion.name.bright_white().bold(),
                format!("({})", companion.language).dimmed(),
                companion.mood.bright_yellow(),
            );
            for line in sprite_text.lines() {
                println!("    {}", line.bright_green());
            }
            println!(
                "    ❤️  {}/{}  ⚡ {}/{}  📊 {}%",
                companion.health,
                companion.max_health,
                companion.energy,
                companion.max_energy,
                companion.code_quality,
            );
            println!();
        }
    }
    println!(
        "  Use {} to create a new companion",
        "vox gamify companion create --name <NAME> --code <FILE>".bright_green()
    );

    Ok(())
}

/// Create a new companion from a source file.
pub async fn companion_create(name: &str, code_file: &std::path::Path) -> Result<()> {
    let code = std::fs::read_to_string(code_file)?;

    let id = vox_runtime::builtins::vox_uuid();

    let user_id = vox_db::paths::local_user_id();
    let mut companion = Companion::new(&id, &user_id, name, "vox");
    companion.code_hash = Some(vox_runtime::builtins::vox_hash_fast(&code));
    companion.description = Some(format!("Created from {}", code_file.display()));

    // Generate sprite (try AI, fall back to deterministic)
    let client = FreeAiClient::auto_discover().await;
    let sprite_text = sprite::generate_ai_sprite(&client, name, "vox", Mood::Neutral).await;
    companion.ascii_sprite = Some(sprite_text.clone());

    let db_conn = get_db().await?;
    db::upsert_companion(&db_conn, &companion).await?;

    // Increment Quests
    let mut profile = match db::get_profile(&db_conn, &user_id).await? {
        Some(p) => p,
        None => GamifyProfile::new_default(&user_id),
    };
    let mut quests = db::list_quests(&db_conn, &user_id).await?;
    for q in &mut quests {
        if q.quest_type == quest::QuestType::Create && q.increment(1) {
            println!(
                "  {} Quest Completed: {}",
                "🌟".bright_yellow(),
                q.description.bright_white()
            );
            profile.add_xp(q.xp_reward);
            profile.add_crystals(q.crystal_reward);
        }
    }
    db::upsert_profile(&db_conn, &profile).await?;
    for q in &quests {
        db::upsert_quest(&db_conn, q).await?;
    }

    println!("{}", "✨ Companion created!".bright_green().bold());
    println!();
    println!(
        "  {} {} [{}]",
        companion.mood.emoji(),
        companion.name.bright_white().bold(),
        companion.language.bright_cyan(),
    );
    for line in sprite_text.lines() {
        println!("    {}", line.bright_green());
    }
    println!();
    println!("  ID: {}", companion.id.dimmed());
    println!(
        "  Code quality: {}%",
        companion.code_quality.to_string().bright_yellow()
    );

    Ok(())
}

/// List daily quests.
pub async fn quest_list() -> Result<()> {
    let db = get_db().await?;

    // Generate deterministic daily quests based on the day
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let day_seed = now / 86_400;

    let user_id = vox_db::paths::local_user_id();
    let mut quests = db::list_quests(&db, &user_id).await?;

    // Filter out expired quests or if none exist for today, generate new ones
    if quests.is_empty() || quests.iter().all(|q| q.is_expired()) {
        quests = quest::generate_daily_quests(&user_id, day_seed);
        for q in &quests {
            db::upsert_quest(&db, q).await?;
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
        println!("    💡 {}", q.hint().dimmed());
        println!();
    }

    Ok(())
}

/// Start a bug battle.
pub async fn battle_start(companion_name: &str) -> Result<()> {
    let db_conn = get_db().await?;

    let user_id = vox_db::paths::local_user_id();

    let companions = db::list_companions(&db_conn, &user_id).await?;
    let companion = match companions.into_iter().find(|c| c.name == companion_name) {
        Some(c) => c,
        None => {
            println!(
                "  ❌ Companion '{}' not found!",
                companion_name.bright_yellow()
            );
            return Ok(());
        }
    };

    // Scan for real bugs!
    use vox_toestub::rules::Severity;
    use vox_toestub::{ToestubConfig, ToestubEngine};

    println!("  {} Searching for bugs in the rift...", "🔍".bright_blue());

    let config = ToestubConfig {
        roots: vec![std::env::current_dir()?],
        min_severity: Severity::Warning,
        excludes: vec![".git".to_string(), "target".to_string()],
        ..Default::default()
    };

    let engine = ToestubEngine::new(config);
    let (scan_result, _) = engine.run_and_report();

    let finding = match scan_result.findings.first() {
        Some(f) => f,
        None => {
            println!("  ✨ No bugs found! Your codebase is too strong today.");
            return Ok(());
        }
    };

    let battle = Battle::from_finding(
        format!(
            "battle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ),
        &user_id,
        companion.id.clone(),
        &finding.rule_id,
        finding.message.clone(),
        Some(String::new()), // Removed code_context usage
    );

    db::insert_battle(&db_conn, &battle).await?;

    println!("{}", "╔══════════════════════════════════╗".bright_red());
    println!("{}", "║       ⚔️  Bug Battle!            ║".bright_red());
    println!("{}", "╚══════════════════════════════════╝".bright_red());
    println!();
    println!(
        "  🐱 {} vs {} {}",
        companion_name.bright_white().bold(),
        battle.bug_type.emoji(),
        battle.bug_type.display().bright_red(),
    );
    println!();
    println!("  {}", battle.bug_description.bright_yellow());
    if let Some(ref code) = battle.bug_code {
        println!();
        println!(
            "  Buggy code location: {}:{}",
            finding.file.display(),
            finding.line
        );
        println!("  Context:");
        for line in code.lines().take(5) {
            println!("    {}", line.bright_red());
        }
    }
    println!();
    println!(
        "  Rewards: 💎{} ⭐{}",
        battle.bug_type.crystal_reward().to_string().bright_yellow(),
        battle.bug_type.xp_reward().to_string().bright_cyan(),
    );
    println!();
    println!(
        "  Submit your fix with: {}",
        format!(
            "vox gamify battle submit --companion {} --code <FILE>",
            companion_name
        )
        .bright_green()
    );

    Ok(())
}

/// Interact with a companion.
pub async fn companion_interact(name: &str, interaction: vox_gamify::Interaction) -> Result<()> {
    let db_conn = get_db().await?;
    let user_id = vox_db::paths::local_user_id();
    let companions = db::list_companions(&db_conn, &user_id).await?;

    let mut companion = match companions.into_iter().find(|c| c.name == name) {
        Some(c) => c,
        None => {
            println!(
                "  ❌ Companion '{}' not found!",
                name.to_string().bright_yellow()
            );
            return Ok(());
        }
    };

    println!("{}", "╔══════════════════════════════════╗".bright_cyan());
    println!("{}", "║      🐾  Interaction!           ║".bright_cyan());
    println!("{}", "╚══════════════════════════════════╝".bright_cyan());
    println!();
    println!(
        "  Interacting with {}...",
        companion.name.bright_white().bold()
    );

    companion.interact(interaction);

    // Regenerate sprite based on new mood if needed
    let client = FreeAiClient::auto_discover().await;
    let sprite_text = sprite::generate_ai_sprite(
        &client,
        &companion.name,
        &companion.language,
        companion.mood,
    )
    .await;
    companion.ascii_sprite = Some(sprite_text.clone());

    db::upsert_companion(&db_conn, &companion).await?;

    match interaction {
        vox_gamify::Interaction::Feed => println!("  🍔 You fed {}!", companion.name),
        vox_gamify::Interaction::Play => println!("  🎾 You played with {}!", companion.name),
        vox_gamify::Interaction::Rest => println!("  💤 {} took a rest.", companion.name),
        _ => println!("  ⚙️ System event triggered for {}.", companion.name),
    }

    println!();
    for line in sprite_text.lines() {
        println!("    {}", line.bright_green());
    }

    println!();
    println!(
        "    {}  {}/{}  ⚡ {}/{}  [{}]",
        companion.mood.emoji(),
        companion.health,
        companion.max_health,
        companion.energy,
        companion.max_energy,
        companion.mood.bright_yellow(),
    );

    Ok(())
}

/// Submit code to win a bug battle.
pub async fn battle_submit(companion_name: &str, code_file: &std::path::Path) -> Result<()> {
    let db_conn = get_db().await?;
    let code = std::fs::read_to_string(code_file)?;

    let user_id = vox_db::paths::local_user_id();
    let mut profile = match db::get_profile(&db_conn, &user_id).await? {
        Some(p) => p,
        None => GamifyProfile::new_default(&user_id),
    };

    let companions = db::list_companions(&db_conn, &user_id).await?;
    let mut companion = match companions.into_iter().find(|c| c.name == companion_name) {
        Some(c) => c,
        None => {
            println!(
                "  ❌ Companion '{}' not found!",
                companion_name.to_string().bright_yellow()
            );
            return Ok(());
        }
    };

    // Find the most recent unfinished battle for this companion
    let battles = db::list_battles(&db_conn, &user_id, 20).await?;
    let mut battle = match battles
        .into_iter()
        .find(|b| b.companion_id == companion.id && !b.success)
    {
        Some(b) => b,
        None => {
            println!(
                "  ❌ No active battle found for {}. Start one with {}!",
                companion_name.bright_white().bold(),
                "vox gamify battle start".bright_green()
            );
            return Ok(());
        }
    };

    println!("{}", "╔══════════════════════════════════╗".bright_green());
    println!("{}", "║      🏆  Battle Result!         ║".bright_green());
    println!("{}", "╚══════════════════════════════════╝".bright_green());
    println!();

    if !companion.spend_battle_energy() {
        println!(
            "  {} {} is too tired to battle! Try resting them.",
            "💤".bright_blue(),
            companion.name.bright_white().bold()
        );
        return Ok(());
    }

    // In a real implementation we would type check the submitted code
    let is_success = !code.contains("todo!()") && !code.is_empty();

    if is_success {
        battle.record_result(true, 60); // Simulated 60s
        battle.submitted_code = Some(code);

        let leveled_up = profile.add_xp(battle.xp_earned);
        profile.add_crystals(battle.crystals_earned);

        // Increment Quests
        let mut quests = db::list_quests(&db_conn, &user_id).await?;
        for q in &mut quests {
            if q.quest_type == quest::QuestType::Battle && q.increment(1) {
                println!(
                    "  {} Quest Completed: {}",
                    "🌟".bright_yellow(),
                    q.description.bright_white()
                );
                profile.add_xp(q.xp_reward);
                profile.add_crystals(q.crystal_reward);
            }
        }

        // Persist everything
        db::upsert_profile(&db_conn, &profile).await?;
        db::upsert_companion(&db_conn, &companion).await?;

        // Update battle record
        db_conn.store().conn.execute(
            "UPDATE gamify_battles SET success = 1, crystals_earned = ?1, xp_earned = ?2, submitted_code = ?3 WHERE id = ?4",
            params![battle.crystals_earned as i64, battle.xp_earned as i64, battle.submitted_code.clone(), battle.id.clone()]
        ).await?;

        for q in &quests {
            db::upsert_quest(&db_conn, q).await?;
        }

        println!(
            "  🎉 You and {} slayed the bug!",
            companion.name.bright_white().bold()
        );
        println!();
        println!("  +{} XP", battle.xp_earned.to_string().bright_cyan());
        println!(
            "  +{} Crystals",
            battle.crystals_earned.to_string().bright_cyan()
        );

        if leveled_up {
            println!(
                "  ⭐ {} You are now level {}",
                "Level Up!".bright_yellow(),
                profile.level.to_string().bright_white()
            );
        }
    } else {
        println!("  ❌ Oh no! The bug fought back.");
        db::upsert_companion(&db_conn, &companion).await?;
        println!("  Make sure your code works and has no `todo!()` remaining.");
    }

    Ok(())
}

/// Render a progress bar.
fn render_progress_bar(pct: f64, width: usize) -> String {
    let filled = (pct * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!(
        "[{}{}]",
        "█".repeat(filled).bright_green(),
        "░".repeat(empty).dimmed(),
    )
}
