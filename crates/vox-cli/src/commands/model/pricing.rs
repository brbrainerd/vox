//! CLI wiring / registry: handler key `commands::model::pricing` (`contracts/cli/command-registry.yaml`).
use clap::{Parser, Subcommand};
use comfy_table::Table;
use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use owo_colors::OwoColorize;
use vox_orchestrator::models::scoring::is_deepseek_off_peak;
use vox_orchestrator::models::{ModelRegistry, PricingSource, ProviderType};
use vox_orchestrator::orchestrator::catalog_refresh::run_foreground_refresh;

#[derive(Parser)]
pub struct PricingArgs {
    #[command(subcommand)]
    pub cmd: PricingCmd,
}

#[derive(Subcommand)]
pub enum PricingCmd {
    /// Show the model pricing catalog (observed vs catalog costs, cache pricing, source)
    Show {
        #[arg(long)]
        model: Option<String>,
    },
    /// Perform batch aggregation of telemetry into the pricing catalog
    Rollup,
    /// Check pricing freshness — flag models still on stale bootstrap pricing
    Check,
    /// Fetch live pricing from OpenRouter + LiteLLM and write to the local cache.
    /// Safe to run without a daemon; updates ~/.vox/cache/model-catalog.v1.json.
    Refresh,
}

pub async fn run(args: PricingArgs) -> anyhow::Result<()> {
    match args.cmd {
        PricingCmd::Show { model } => run_show(model).await,
        PricingCmd::Rollup => run_rollup().await,
        PricingCmd::Check => run_check().await,
        PricingCmd::Refresh => run_refresh().await,
    }
}

async fn run_show(model_filter: Option<String>) -> anyhow::Result<()> {
    let db = vox_db::VoxDb::open_default().await?;
    let pricing = db.get_pricing_catalog().await?;

    // Load the registry from cache only (no network call) so we can join in
    // cache pricing, prompt-caching support, and the pricing_source field.
    let registry = ModelRegistry::from_cache();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);
    table.set_header(vec![
        "Model",
        "Provider",
        "Obs $/1K",
        "Cat in $/1K",
        "Cache-hit $/1K",
        "Caching?",
        "Source",
        "Conf",
        "Samples",
    ]);

    for row in &pricing {
        if let Some(ref m) = model_filter {
            if !row.model_id.contains(m) {
                continue;
            }
        }

        let obs_str = match row.observed_blended_per_1k {
            Some(v) => format!("{:.6}", v),
            None => "-".to_string(),
        };

        let cat_str = format!("{:.6}", row.catalog_input_per_1k);

        // Supplement with live registry data (cache pricing + source).
        let (cache_hit_str, caching_str, source_str) =
            if let Some(spec) = registry.get(&row.model_id) {
                let cache_hit = if spec.cache_read_cost_per_1k > 0.0 {
                    format!("{:.6}", spec.cache_read_cost_per_1k)
                } else {
                    "-".to_string()
                };
                let caching = if spec.supports_prompt_caching {
                    "yes".green().to_string()
                } else {
                    "no".dimmed().to_string()
                };
                let source = format_pricing_source(&spec.pricing_source);
                (cache_hit, caching, source)
            } else {
                (
                    "-".to_string(),
                    "-".dimmed().to_string(),
                    "-".dimmed().to_string(),
                )
            };

        let conf_colored = match row.confidence.as_str() {
            "high" => "high".green().to_string(),
            "medium" => "medium".yellow().to_string(),
            _ => "low".dimmed().to_string(),
        };

        table.add_row(vec![
            row.model_id.clone(),
            row.provider.clone(),
            obs_str,
            cat_str,
            cache_hit_str,
            caching_str,
            source_str,
            conf_colored,
            row.n_provider_reported.to_string(),
        ]);
    }

    if table.row_iter().count() == 0 {
        println!("No pricing data available. Try running `vox model pricing rollup` first.");
    } else {
        println!("{table}");
    }

    Ok(())
}

async fn run_rollup() -> anyhow::Result<()> {
    let db = vox_db::VoxDb::open_default().await?;
    println!("Rolling up observed telemetry into pricing catalog...");

    let changes = db.rollup_pricing_catalog().await?;
    println!("{} rows updated.", changes.to_string().green());

    Ok(())
}

/// Scan the cached model registry and flag models still priced from the bootstrap JSON.
/// These may have stale costs that haven't been overwritten by a live refresh yet.
async fn run_check() -> anyhow::Result<()> {
    let registry = ModelRegistry::from_cache();
    let all = registry.list_models();

    let stale: Vec<_> = all
        .iter()
        .filter(|m| m.pricing_source == PricingSource::Bootstrap && !m.is_free)
        .collect();

    let caching_ready: Vec<_> = all.iter().filter(|m| m.supports_prompt_caching).collect();

    if stale.is_empty() {
        println!("{}", "✓ No paid models on stale bootstrap pricing.".green());
    } else {
        println!(
            "{} paid model(s) still on bootstrap pricing (may be stale):\n",
            stale.len().to_string().yellow()
        );
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS);
        table.set_header(vec!["Model", "Provider", "$/1K (blended)", "Source"]);
        for m in &stale {
            table.add_row(vec![
                m.id.clone(),
                m.provider.clone(),
                format!("{:.6}", m.cost_per_1k),
                format_pricing_source(&m.pricing_source),
            ]);
        }
        println!("{table}");
        println!(
            "\nRun `vox model pricing refresh` (or `vox model pricing rollup`, or restart the daemon) to trigger a live refresh."
        );
    }

    println!(
        "\n{} model(s) support prompt caching (cache-hit pricing available):",
        caching_ready.len().to_string().cyan()
    );
    for m in &caching_ready {
        println!(
            "  {} — cache-hit {}, cache-create {}",
            m.id.cyan(),
            format!("{:.6} $/1K", m.cache_read_cost_per_1k).green(),
            format!("{:.6} $/1K", m.cache_creation_cost_per_1k).yellow(),
        );
    }

    // ── DeepSeek off-peak window status ──────────────────────────────────────
    let deepseek_models: Vec<_> = all
        .iter()
        .filter(|m| matches!(m.provider_type, ProviderType::DeepSeek))
        .collect();

    if !deepseek_models.is_empty() {
        let off_peak = is_deepseek_off_peak();
        let window_str = "UTC 16:30–00:30";
        let status = if off_peak {
            "✓ ACTIVE now".green().to_string()
        } else {
            "✗ inactive".dimmed().to_string()
        };
        println!(
            "\n{} DeepSeek off-peak discount window ({window_str}): {status}",
            "●".cyan()
        );
        for m in &deepseek_models {
            let is_r1 = m.id.to_ascii_lowercase().contains("r1");
            let discount_pct = if is_r1 { "75%" } else { "50%" };
            let discount_str = if off_peak {
                format!("{discount_pct} off").green().to_string()
            } else {
                format!("{discount_pct} off when active")
                    .dimmed()
                    .to_string()
            };
            println!("  {} — {discount_str}", m.id.cyan());
        }
    }

    Ok(())
}

async fn run_refresh() -> anyhow::Result<()> {
    use owo_colors::OwoColorize;
    println!("Fetching live pricing from OpenRouter, LiteLLM, and Anthropic (if key set)...");
    let report = run_foreground_refresh().await?;
    println!(
        "✓ Refreshed: {} OpenRouter models, {} LiteLLM entries, {} Anthropic-direct models",
        report.openrouter_count.to_string().green(),
        report.litellm_count.to_string().green(),
        report.anthropic_count.to_string().green(),
    );
    println!(
        "  {} total models written to {}",
        report.total_written.to_string().cyan(),
        report.cache_path.display()
    );
    Ok(())
}

fn format_pricing_source(source: &PricingSource) -> String {
    match source {
        PricingSource::Bootstrap => "bootstrap".dimmed().to_string(),
        PricingSource::OpenRouter => "openrouter".cyan().to_string(),
        PricingSource::AnthropicDirect => "anthropic-api".cyan().to_string(),
        PricingSource::LiteLLM => "litellm".blue().to_string(),
        PricingSource::UserConfig => "user-config".magenta().to_string(),
        PricingSource::Telemetry => "telemetry".green().to_string(),
        PricingSource::Unknown => "unknown".red().to_string(),
    }
}
