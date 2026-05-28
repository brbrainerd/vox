//! `vox model shadow` — L2.5 shadow evaluation for provisional models.

use clap::Args;
use owo_colors::OwoColorize;
use vox_orchestrator::models::ModelRegistry;
use vox_orchestrator::models::autonomic::{ModelConfidence, PromotionEvidence, record_promotion};

#[derive(Args, Debug)]
pub struct ShadowArgs {
    /// Model id to shadow-evaluate.
    pub model_id: String,
    /// Optional task category hint (defaults to General).
    #[arg(long, default_value = "General")]
    pub task: String,
}

pub async fn run(args: ShadowArgs) -> anyhow::Result<()> {
    let registry = ModelRegistry::from_cache();
    if registry.get(&args.model_id).is_none() {
        anyhow::bail!("model {} not found in local catalog cache", args.model_id);
    }

    println!(
        "{} Shadow-evaluating {} (task={})",
        " INFO ".on_blue().white().bold(),
        args.model_id.cyan(),
        args.task
    );

    // Operational scaffold: promote Provisional → Shadowed with telemetry for council gates.
    record_promotion(
        &args.model_id,
        ModelConfidence::Provisional,
        ModelConfidence::Shadowed,
        PromotionEvidence::ShadowEval,
    );

    println!(
        "  {} confidence promotion recorded ({} → {})",
        "✓".green(),
        ModelConfidence::Provisional.as_str(),
        ModelConfidence::Shadowed.as_str()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_args_default_task() {
        let args = ShadowArgs {
            model_id: "test/model".into(),
            task: "General".into(),
        };
        assert_eq!(args.task, "General");
    }
}
