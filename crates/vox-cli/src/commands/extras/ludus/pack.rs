//! Lex Pack listing and initialization.

use anyhow::Result;
use owo_colors::OwoColorize;

/// List project-specific Lex Pack rules.
pub async fn pack_list() -> Result<()> {
    // Try to load Lex Pack from project root
    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let pack_path = root.join(vox_config::paths::REPO_LUDUS_PACK_FILE);

    println!(
        "{}",
        "╔══════════════════════════════════╗".bright_magenta()
    );
    println!("{}", "║       📦 Lex Pack Rules         ║".bright_magenta());
    println!(
        "{}",
        "╚══════════════════════════════════╝".bright_magenta()
    );
    println!();

    if pack_path.exists() {
        match vox_gamify::lex_pack::load_lex_pack(&pack_path) {
            Ok(pack) => {
                println!(
                    "  {} v{}",
                    pack.name.bright_white().bold(),
                    pack.version.bright_cyan()
                );
                println!(
                    "  {}",
                    pack.description.as_deref().unwrap_or("").italic().dimmed()
                );
                println!();

                if !pack.glyphs.is_empty() {
                    println!("  Custom Glyphs:");
                    for g in pack.glyphs {
                        println!(
                            "    {} {} [{}]",
                            g.icon,
                            g.name.bright_white(),
                            g.trigger_event.dimmed()
                        );
                    }
                }

                if !pack.lumens_weights.is_empty() {
                    println!("\n  Lumen Weights:");
                    for lw in pack.lumens_weights {
                        println!(
                            "    {:<20} {:>+4} ✦",
                            lw.event_type.dimmed(),
                            lw.lumens_delta.to_string().bright_yellow()
                        );
                    }
                }
            }
            Err(e) => {
                println!("  ❌ Error loading Lex Pack: {}", e);
            }
        }
    } else {
        println!("  No active Lex Pack found for this project.");
        println!(
            "  Run {} to create one.",
            "vox ludus pack init".bright_green()
        );
    }

    Ok(())
}

/// Initialize a new Lex Pack.
pub async fn pack_init(template: &str) -> Result<()> {
    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let ludus_dir = root.join(vox_config::paths::REPO_LUDUS_DIR);
    if !ludus_dir.exists() {
        std::fs::create_dir_all(&ludus_dir)?;
    }

    let pack_path = ludus_dir.join("lex-pack.toml");
    if pack_path.exists() {
        println!("  ❌ Lex Pack already exists in this project.");
        return Ok(());
    }

    let toml_content = match template {
        "core" => {
            r#"[pack]
id = "project-core"
name = "Core Rules"
description = "Project-specific rewards and quality gates"
version = "0.1.0"

[[glyphs]]
id = "test-commander"
name = "Test Commander"
description = "Write 10 new passing tests in one day"
icon = "🎖️"
trigger_event = "test_pass"
trigger_count = 10
xp_reward = 100

[[lumens_weights]]
event_type = "toestub_clean"
lumens_delta = 5
"#
        }
        _ => anyhow::bail!("Unknown template '{}'", template),
    };

    std::fs::write(&pack_path, toml_content)?;
    println!(
        "  ✅ {} initialized at {}",
        "Lex Pack".bright_green(),
        pack_path.display().dimmed()
    );

    Ok(())
}
