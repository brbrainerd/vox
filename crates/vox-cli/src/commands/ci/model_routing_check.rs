use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::path::Path;

pub fn run(root: &Path) -> Result<()> {
    println!("{}", "Checking model-routing.v1.yaml contract...".cyan());

    let yaml_path = root.join("contracts/orchestration/model-routing.v1.yaml");
    if !yaml_path.exists() {
        anyhow::bail!("Missing model-routing.v1.yaml at {}", yaml_path.display());
    }

    let contents =
        std::fs::read_to_string(&yaml_path).context("Failed to read model-routing.v1.yaml")?;

    let config: vox_config::ModelRoutingConfig = serde_yaml::from_str(&contents)
        .context("Failed to parse model-routing.v1.yaml against the ModelRoutingConfig schema")?;

    println!("{} Parsed successfully.", "✓".green());

    if config.exploration.budget_usd_per_day <= 0.0 {
        anyhow::bail!("exploration.budget_usd_per_day must be > 0.0");
    }
    println!(
        "{} Exploration budget is sane: ${:.2}/day",
        "✓".green(),
        config.exploration.budget_usd_per_day
    );

    if config.latency_bands.excellent_ms >= config.latency_bands.poor_ms {
        anyhow::bail!("latency_bands.excellent_ms must be strictly less than poor_ms");
    }

    let qw = &config.quality_weights;
    let qw_sum = qw.socrates_factuality
        + qw.contradiction_inverse
        + qw.success_rate
        + qw.p50_latency_inverse
        + qw.cost_inverse;
    if qw_sum <= 0.0 {
        anyhow::bail!("quality_weights must sum to a positive value");
    }
    println!(
        "{} quality_weights present (sum={:.2})",
        "✓".green(),
        qw_sum
    );

    if config.safety.max_cost_usd_per_request <= 0.0 {
        anyhow::bail!("safety.max_cost_usd_per_request must be > 0.0");
    }

    // Premium alias drift guard: pins.yaml is SSOT; routing.yaml aliases must not diverge.
    if let Some(pins) = vox_config::load_model_pins_config() {
        let mut drift = Vec::new();
        for (k, v) in &config.premium_alias {
            if let Some(pin_v) = pins.premium_alias.get(k)
                && pin_v != v
            {
                drift.push(format!("{k}: routing={v} pins={pin_v}"));
            }
        }
        if !drift.is_empty() {
            anyhow::bail!(
                "premium_alias drift between model-routing.v1.yaml and model-pins.v1.yaml: {}",
                drift.join("; ")
            );
        }
        println!(
            "{} premium_alias keys aligned with model-pins.v1.yaml",
            "✓".green()
        );
    }

    // Retired ids must never appear as live premium aliases.
    if let Some(pins) = vox_config::load_model_pins_config() {
        let retired: HashSet<&str> = pins.retired_ids.iter().map(String::as_str).collect();
        for (k, v) in &config.premium_alias {
            if retired.contains(v.as_str()) {
                anyhow::bail!(
                    "premium_alias {k} -> {v} references a retired model id from model-pins.v1.yaml"
                );
            }
        }
    }

    // Runtime wiring guard: scorer must reference quality_weights symbol.
    let scoring_rs = root.join("crates/vox-orchestrator/src/models/scoring.rs");
    let scoring_src = std::fs::read_to_string(&scoring_rs).with_context(|| {
        format!(
            "Failed to read {} for SSOT wiring guard",
            scoring_rs.display()
        )
    })?;
    if !scoring_src.contains("scoreboard_feedback_boost")
        || !scoring_src.contains("quality_weights")
    {
        anyhow::bail!(
            "scoring.rs missing quality_weights/scoreboard_feedback_boost wiring (SSOT drift)"
        );
    }
    println!(
        "{} orchestrator scoring references quality_weights",
        "✓".green()
    );

    println!("{} Model routing contract is valid.", "PASS".green().bold());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_routing_check_passes_on_repo_contract() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        run(&root).expect("model routing SSOT check should pass");
    }
}
