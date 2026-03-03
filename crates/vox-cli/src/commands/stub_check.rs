use std::path::PathBuf;

use owo_colors::OwoColorize;
use vox_toestub::rules::{Language, Severity};
use vox_toestub::{OutputFormat, ToestubConfig, ToestubEngine};

use vox_db::VoxDb;
use vox_gamify::{db, profile::GamifyProfile};

/// Run the TOESTUB analysis.
pub async fn run(
    path: &std::path::Path,
    format: Option<&str>,
    severity: Option<&str>,
    suggest_fixes: bool,
    rules: Option<&str>,
    excludes: &[String],
    langs: Option<&str>,
) -> anyhow::Result<()> {
    let config = ToestubConfig {
        roots: vec![PathBuf::from(path)],
        min_severity: match severity.unwrap_or("warning") {
            "info" => Severity::Info,
            "error" => Severity::Error,
            "critical" => Severity::Critical,
            _ => Severity::Warning,
        },
        format: OutputFormat::parse_format(format.unwrap_or("terminal")),
        suggest_fixes,
        languages: langs.map(|l| {
            l.split(',')
                .filter_map(|s| match s.trim() {
                    "rust" | "rs" => Some(Language::Rust),
                    "ts" | "typescript" => Some(Language::TypeScript),
                    "python" | "py" => Some(Language::Python),
                    "gdscript" | "gd" => Some(Language::GDScript),
                    "vox" => Some(Language::Vox),
                    _ => None,
                })
                .collect()
        }),
        excludes: excludes.to_vec(),
        rule_filter: rules.map(|r| r.split(',').map(|s| s.trim().to_string()).collect()),
        schema_path: std::fs::canonicalize("vox-schema.json").ok(),
        ..Default::default()
    };

    let engine = ToestubEngine::new(config);
    let (result, output) = engine.run_and_report();

    // Print the formatted output
    println!("{}", output);

    // Print summary footer
    let summary = result.summary();
    if result.findings.is_empty() {
        println!(
            "{}",
            "🦶 TOESTUB: All clear — no issues found.".green().bold()
        );
    } else {
        println!(
            "{} Scanned {} files with {} rules, found {} issues.",
            "🦶 TOESTUB:".bold(),
            result.files_scanned,
            result.rules_applied,
            result.findings.len(),
        );
        if summary.critical > 0 || summary.error > 0 {
            println!(
                "{}",
                format!(
                    "   ⚠  {} critical, {} errors require attention.",
                    summary.critical, summary.error,
                )
                .red()
            );
        }
    }

    // If fix suggestions were requested, also dump the task queue
    if suggest_fixes && !result.task_queue.fix_suggestions.is_empty() {
        println!("\n{}", result.task_queue.to_markdown_checklist());
    }

    // ── Gamification Auto-Rewards ──
    if result.findings.is_empty() {
        // Reward the user for a clean codebase!
        if let Ok(db) = VoxDb::connect_default().await {
            let user_id = vox_db::paths::local_user_id();
            let mut profile = match db::get_profile(&db, &user_id).await.unwrap_or(None) {
                Some(p) => p,
                None => {
                    let p = GamifyProfile::new_default(&user_id);
                    db::upsert_profile(&db, &p).await.ok();
                    p
                }
            };

            let mut xp_gain = 10;
            let mut crystal_gain = 5;

            if let Ok(Some(raw)) = db
                .store()
                .get_user_preference(&user_id, "gamify.clean_run_xp")
                .await
            {
                if let Ok(val) = raw.parse::<u64>() {
                    xp_gain = val;
                }
            }
            if let Ok(Some(raw)) = db
                .store()
                .get_user_preference(&user_id, "gamify.clean_run_crystals")
                .await
            {
                if let Ok(val) = raw.parse::<u64>() {
                    crystal_gain = val;
                }
            }

            let leveled_up = profile.add_xp(xp_gain);
            profile.add_crystals(crystal_gain);

            if db::upsert_profile(&db, &profile).await.is_ok() {
                println!();
                println!("{}", "🎉 Gamification Rewards!".bright_yellow());
                println!("  +{} XP", xp_gain.to_string().bright_cyan());
                println!("  +{} Crystals", crystal_gain.to_string().bright_cyan());

                if leveled_up {
                    println!(
                        "  {} Level Up! You are now level {}",
                        "⭐".bright_yellow(),
                        profile.level.to_string().bright_white()
                    );
                }
            }
        }
    } else {
        println!(
            "\n  {} Want extra rewards? Run {} to fight these bugs in a battle.",
            "🔮".bright_magenta(),
            "vox gamify battle start".bright_green()
        );
    }

    // Return error exit code if there are error-level findings
    if result.has_errors() {
        anyhow::bail!(
            "TOESTUB found {} error-level issues.",
            summary.error + summary.critical
        );
    }

    Ok(())
}
