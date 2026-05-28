use std::path::Path;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;

pub fn run(root: &Path) -> Result<()> {
    println!("{}", "Checking model-routing.v1.yaml contract...".cyan());

    let yaml_path = root.join("contracts/orchestration/model-routing.v1.yaml");
    if !yaml_path.exists() {
        anyhow::bail!("Missing model-routing.v1.yaml at {}", yaml_path.display());
    }

    let contents = std::fs::read_to_string(&yaml_path)
        .context("Failed to read model-routing.v1.yaml")?;
        
    let config: vox_config::ModelRoutingConfig = serde_yaml::from_str(&contents)
        .context("Failed to parse model-routing.v1.yaml against the ModelRoutingConfig schema")?;

    println!("{} Parsed successfully.", "✓".green());
    
    if config.exploration.budget_usd_per_day <= 0.0 {
        anyhow::bail!("exploration.budget_usd_per_day must be > 0.0");
    }
    println!("{} Exploration budget is sane: ${:.2}/day", "✓".green(), config.exploration.budget_usd_per_day);
    
    if config.latency_bands.excellent_ms >= config.latency_bands.poor_ms {
        anyhow::bail!("latency_bands.excellent_ms must be strictly less than poor_ms");
    }

    println!("{} {}", "PASS".green().bold(), "Model routing contract is valid.");
    Ok(())
}
