use clap::Parser;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::models::{ModelRegistry, ModelScore};
use vox_orchestrator::types::TaskCategory;

/// Explain model selection for a given task description.
#[derive(Parser)]
pub struct ExplainArgs {
    /// Task description or prompt.
    pub task: String,
    /// Explicit task category (optional).
    #[arg(long)]
    pub category: Option<String>,
    /// Estimated complexity (1-10).
    #[arg(long, default_value_t = 5)]
    pub complexity: u8,
    /// Also show the free-tier router's ranked candidates with the human-readable
    /// rationale for each (the same rationale the MCP selection path emits).
    #[arg(long)]
    pub free_tier: bool,
    /// (with --free-tier) Prioritize ultra-low-latency free models.
    #[arg(long)]
    pub latency_critical: bool,
    /// (with --free-tier) Request fill-in-the-middle-capable free models.
    #[arg(long)]
    pub fim: bool,
}

/// Pure helper: rank free models via the FreeTierRouter and return each
/// candidate's id paired with its rationale. No I/O — unit-testable.
fn render_free_tier(
    req: &vox_dei_shim::selection::FreeTierRouteRequest,
    models: &[vox_orchestrator::models::ModelSpec],
) -> Vec<(String, &'static str)> {
    vox_dei_shim::selection::FreeTierRouter::new()
        .route(req, models)
        .into_iter()
        .map(|c| (c.model.id, c.rationale))
        .collect()
}

pub async fn run(args: ExplainArgs) -> anyhow::Result<()> {
    // 1. Setup Registry
    let mut registry = ModelRegistry::new();

    // 2. Load Scoreboard from DB
    let db_config = DbConfig::resolve_canonical().map_err(anyhow::Error::msg)?;
    let db = VoxDb::connect(db_config).await?;
    let db_scores = db.get_model_scoreboard(7).await?;

    let mut scores = HashMap::new();
    for row in db_scores {
        scores.insert(row.model_id.clone(), ModelScore::from(row));
    }
    registry.inject_scoreboard(scores);

    // 3. Construct simulation parameters
    let category = if let Some(cat_str) = args.category {
        use std::str::FromStr;
        TaskCategory::from_str(&cat_str).unwrap_or(TaskCategory::General)
    } else {
        TaskCategory::General
    };

    let complexity = args.complexity;
    let description = args.task;

    // 4. Run Selection Explain
    println!(
        "{} Model Selection for task: \"{}\"",
        " EXPLAIN ".on_blue().white().bold(),
        description.italic()
    );
    println!("Category: {:?}, Complexity: {}", category, complexity);
    let snap =
        vox_actor_runtime::route_capability_policy::RouteCapabilityPolicySnapshot::from_env();
    println!(
        "Route policy profile: {} (net={}, provider_net={}, local_http={})",
        snap.profile, snap.allow_net, snap.allow_provider_network, snap.allow_local_model_http
    );
    let exclusions = registry.explain_route_policy_exclusions();
    if !exclusions.is_empty() {
        println!("{}", " Policy exclusions (VOX_ROUTE_*):".yellow().bold());
        for (id, reason) in exclusions.iter().take(25) {
            println!("  - {}: {}", id.dimmed(), reason);
        }
        if exclusions.len() > 25 {
            println!("  … {} more", exclusions.len() - 25);
        }
    }
    println!("---");

    let strength = vox_orchestrator::models::task_category_strength(category);
    let candidates = registry.explain_selection(
        category,
        strength,
        vox_orchestrator::config::CostPreference::Performance,
    );

    if candidates.is_empty() {
        println!("{}", "❌ No suitable models found in registry.".red());
        return Ok(());
    }

    println!(
        "{} Top Candidates (sorted by priority score):",
        " RANK ".on_green().black().bold()
    );
    for (i, entry) in candidates.iter().take(5).enumerate() {
        let prefix = if i == 0 {
            "🥇"
        } else if i == 1 {
            "🥈"
        } else if i == 2 {
            "🥉"
        } else {
            "  "
        };

        let mut details = Vec::new();
        details.push(format!("Tier: {:?}", entry.capabilities.tier));

        if let Some(score) = registry.get_score(&entry.id) {
            details.push(format!("Success: {:.1}%", score.success_rate * 100.0));
            details.push(format!("Quality: {:.2}", score.quality_score));
        }

        println!("{} {}: {}", prefix, entry.id.bold(), details.join(", "));
    }

    println!("\nSelection: {}", candidates[0].id.green().bold());

    // 4b. Free-tier router rationale (opt-in) — surfaces WHY each free model is
    // chosen, using the same FreeTierRouter the MCP selection path uses.
    if args.free_tier {
        let req = vox_dei_shim::selection::FreeTierRouteRequest {
            task: category,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: args.fim,
            latency_critical: args.latency_critical,
            max_candidates: 5,
        };
        let ranked = render_free_tier(&req, &registry.free_models());
        println!(
            "\n{} Free-tier candidates (with rationale):",
            " FREE ".on_cyan().black().bold()
        );
        if ranked.is_empty() {
            println!("  (no free models satisfy the request)");
        } else {
            for (id, rationale) in &ranked {
                println!("  - {}: {}", id.bold(), rationale.dimmed());
            }
        }
    }

    // 5. Show most recent trace ID
    if let Ok(Some(tid)) = db
        .get_last_interaction_trace_id(&category.to_string())
        .await
    {
        println!("Recent Trace ID: {}", tid.dimmed());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::render_free_tier;
    use vox_dei_shim::selection::FreeTierRouteRequest;
    use vox_orchestrator::models::{
        ModelCapabilities, ModelSpec, ModelTier, PricingSource, ProviderType, StrengthTag,
    };
    use vox_orchestrator::types::TaskCategory;

    fn free_spec(id: &str, provider_type: ProviderType, tier: ModelTier) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: String::new(),
            provider: "test".to_string(),
            provider_type,
            max_tokens: 32_000,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            observed_cost_per_1k: None,
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::OpenRouter,
            is_free: true,
            strengths: vec![StrengthTag::Generalist],
            capabilities: ModelCapabilities {
                max_context: 32_000,
                tier,
                ..Default::default()
            },
            supported_parameters: vec![],
        }
    }

    #[test]
    fn render_free_tier_returns_ids_with_nonempty_rationale() {
        let models = vec![
            free_spec("groq/fast", ProviderType::Groq, ModelTier::Fast),
            free_spec("or/pro", ProviderType::OpenRouter, ModelTier::Pro),
        ];
        let req = FreeTierRouteRequest {
            task: TaskCategory::CodeGen,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: false,
            latency_critical: true,
            max_candidates: 5,
        };
        let ranked = render_free_tier(&req, &models);
        assert!(!ranked.is_empty(), "free models should be ranked");
        for (id, rationale) in &ranked {
            assert!(!rationale.is_empty(), "every candidate must carry a rationale ({id})");
        }
        // The Fast-tier model under latency_critical earns the low-latency rationale.
        let fast = ranked.iter().find(|(id, _)| id == "groq/fast").expect("fast present");
        assert_eq!(fast.1, "Candidate selected for ultra-low latency");
    }

    #[test]
    fn render_free_tier_empty_when_no_free_models() {
        let req = FreeTierRouteRequest {
            task: TaskCategory::CodeGen,
            context_tokens: 0,
            requires_vision: false,
            requires_structured_output: false,
            requires_fill_in_middle: false,
            latency_critical: false,
            max_candidates: 5,
        };
        assert!(render_free_tier(&req, &[]).is_empty());
    }
}
