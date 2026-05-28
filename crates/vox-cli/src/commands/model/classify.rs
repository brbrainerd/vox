//! `vox model classify` — L2 of the model-autonomic system.
//!
//! Build the classifier prompt for a given model id and print it (default) or
//! invoke the classifier LLM (when `--invoke` is set and a classifier key is
//! configured). The classifier itself is read from
//! `contracts/orchestration/model-pins.v1.yaml` (`classifier.primary`).
//!
//! See `docs/src/architecture/model-autonomic-system-2026.md` §3.

use anyhow::{Context, Result};
use clap::Args;
use vox_orchestrator::models::ModelRegistry;
use vox_orchestrator::models::autonomic::{build_classifier_prompt, record_classification};

#[derive(Args, Debug)]
pub struct ClassifyArgs {
    /// Model id to classify (e.g. `anthropic/claude-future-1`).
    pub model_id: String,

    /// Print the classifier prompt that *would* be sent, without calling
    /// the classifier LLM. Default mode — safe to run offline.
    #[arg(long, default_value_t = true)]
    pub dry_run: bool,

    /// Override the classifier model id (defaults to
    /// `model-pins.v1.yaml::classifier.primary`).
    #[arg(long)]
    pub classifier: Option<String>,
}

pub async fn run(args: ClassifyArgs) -> Result<()> {
    let registry = ModelRegistry::from_cache();
    let pins = vox_config::load_model_pins_config().unwrap_or_default();
    let classifier_id = args
        .classifier
        .clone()
        .or(pins.classifier.primary)
        .context("no classifier model configured (set --classifier or pins.yaml::classifier.primary)")?;

    // Pull what we know about the target model from the local catalog, if present.
    let (description, supported_params, sample_cost): (Option<String>, Vec<String>, Option<f64>) =
        match registry.get(&args.model_id) {
            Some(spec) => {
                let cost = if spec.cost_per_1k_input > 0.0 {
                    Some(spec.cost_per_1k_input)
                } else if spec.cost_per_1k > 0.0 {
                    Some(spec.cost_per_1k)
                } else {
                    None
                };
                // ModelSpec doesn't currently surface `description` or
                // `supported_parameters`; the autonomic prompt is intentionally
                // tolerant of either being empty.
                (None, Vec::new(), cost)
            }
            None => (None, Vec::new(), None),
        };

    let prompt = build_classifier_prompt(
        &args.model_id,
        description.as_deref(),
        &supported_params,
        sample_cost,
    );

    if args.dry_run {
        println!("# vox model classify — dry-run\n");
        println!("Classifier (primary pin): {classifier_id}");
        println!("Target model:             {}", args.model_id);
        println!("\n----- PROMPT BEGIN -----\n{prompt}\n----- PROMPT END -----");
        println!(
            "\nNote: run with `--no-dry-run` to actually invoke the classifier. Live invocation is gated until the council ratifies promotion thresholds (see model-autonomic-system-2026.md §6 Phase F)."
        );
        return Ok(());
    }

    // Live-invocation path is scaffolded but disabled by default until the
    // council ratifies budget + thresholds. Emit a placeholder telemetry
    // record so downstream consumers see the call signal even in scaffold mode.
    eprintln!(
        "Live classifier invocation not yet wired (council gate). Emitting a placeholder ClassificationEvent so downstream consumers can verify the telemetry pipeline."
    );
    let placeholder = vox_orchestrator::models::autonomic::ClassificationJudgement {
        tier: vox_orchestrator::models::ModelTier::Unknown,
        strengths: Vec::new(),
        confidence: 0.0,
        rationale: Some("scaffold placeholder — live invocation not yet enabled".into()),
    };
    record_classification(&args.model_id, &classifier_id, &placeholder);
    Ok(())
}
