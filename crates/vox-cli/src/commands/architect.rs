use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::Path;
use vox_pm::VoxWorkspace;
use vox_toestub::{ToestubConfig, ToestubEngine, rules::Severity};

pub async fn run(action: crate::cli_actions::ArchitectAction) -> Result<()> {
    match action {
        crate::cli_actions::ArchitectAction::Check => check_architecture().await,
        crate::cli_actions::ArchitectAction::FixSprawl { apply } => fix_sprawl(apply).await,
        crate::cli_actions::ArchitectAction::Analyze { path } => analyze_god_objects(&path).await,
    }
}

async fn check_architecture() -> Result<()> {
    println!("{}", "🦶 Architect: Checking workspace compliance...".bold().cyan());

    let current_dir = std::env::current_dir()?;
    let workspace = VoxWorkspace::load(&current_dir)
        .context("Failed to load Vox workspace. Are you in a project root?")?;

    let violations = workspace.validate_architecture().map_err(|e| anyhow::anyhow!(e))?;

    if violations.is_empty() {
        println!("{}", "✓ Workspace architecture is compliant with vox-schema.json".green());
    } else {
        println!("{}", "⚠ Architectural violations found:".yellow().bold());
        for violation in violations {
            println!("  - {}", violation.red());
        }
        anyhow::bail!("Architectural check failed");
    }

    Ok(())
}

async fn fix_sprawl(apply: bool) -> Result<()> {
    println!("{}", "🦶 Architect: Scanning for sprawl...".bold().cyan());

    let current_dir = std::env::current_dir()?;
    let workspace = VoxWorkspace::load(&current_dir).map_err(|e| anyhow::anyhow!(e))?;
    let violations = workspace.validate_architecture().map_err(|e| anyhow::anyhow!(e))?;

    if violations.is_empty() {
        println!("{}", "✓ No sprawl detected. Everything is in its right place.".green());
        return Ok(());
    }

    let mut moved_count = 0;
    for violation in violations {
        // Crude parsing of violation message: "Crate 'name' is in 'current', but vox-schema.json expects it under 'expected'"
        if violation.contains("expects it under") {
            let parts: Vec<&str> = violation.split('\'').collect();
            if parts.len() >= 6 {
                let name = parts[1];
                let current_str = parts[3];
                let expected_str = parts[5];

                let current = Path::new(current_str);
                let expected = Path::new(expected_str);

                println!("  {} Crate '{}' is misplaced.", "→".yellow(), name);
                println!("    Current:  {}", current.display().dimmed());
                println!("    Expected: {}", expected.display().green());

                if apply {
                    if let Some(parent) = expected.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::rename(current, expected)?;
                    println!("    {}", "✓ Moved successfully.".green());
                    moved_count += 1;
                }
            }
        }
    }

    if !apply && moved_count == 0 {
        println!("\n{}", "To apply these changes, run with: vox architect fix-sprawl --apply".bold().magenta());
    } else if apply {
        println!("\n{}", format!("✓ Successfully moved {} crate(s).", moved_count).green().bold());
    }

    Ok(())
}

async fn analyze_god_objects(path: &Path) -> Result<()> {
    println!("{}", format!("🦶 Architect: Analyzing God Objects in {}...", path.display()).bold().cyan());

    let config = ToestubConfig {
        roots: vec![path.to_path_buf()],
        min_severity: Severity::Warning,
        rule_filter: Some(vec!["arch/god_object".to_string(), "arch/stub".to_string(), "arch/empty_body".to_string(), "arch/dry_violation".to_string()]),
        schema_path: std::fs::canonicalize("vox-schema.json").ok(),
        ..ToestubConfig::default()
    };

    let engine = ToestubEngine::new(config);
    let (result, report) = engine.run_and_report();

    if result.findings.is_empty() {
        println!("{}", "✓ No God Objects detected. The focus is sharp.".green());
    } else {
        println!("{}", report);
        println!("\n{}", "💡 Tip: Decouple oversized files into traits or sub-modules.".dimmed());
    }

    Ok(())
}
