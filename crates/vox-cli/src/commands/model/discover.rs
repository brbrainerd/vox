use clap::Parser;
use owo_colors::OwoColorize;
use vox_orchestrator::orchestrator::catalog_refresh::run_unified_catalog_refresh;

/// Refresh the model catalog from all sources.
#[derive(Parser)]
pub struct DiscoverArgs {
    /// Force refresh even if cache is warm.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(args: DiscoverArgs) -> anyhow::Result<()> {
    println!(
        "{} Discovering models...",
        " INFO ".on_blue().white().bold()
    );

    let report = run_unified_catalog_refresh(args.force).await?;

    println!(
        "  ✅ OpenRouter: {} models",
        report.openrouter_count.to_string().green()
    );
    println!(
        "  ✅ Ollama: {} models",
        report.ollama_count.to_string().green()
    );
    println!(
        "  ✅ Hugging Face: {} models",
        report.huggingface_count.to_string().green()
    );
    println!(
        "  ✅ Populi mesh: {} models",
        report.mesh_count.to_string().green()
    );
    println!(
        "  ✅ MENS local: {} models",
        report.mens_count.to_string().green()
    );
    if !report.new_discovery_ids.is_empty() {
        println!(
            "    {} {} new model id(s) emitted to telemetry",
            "↳".cyan(),
            report.new_discovery_ids.len().to_string().yellow().bold()
        );
    }

    println!(
        "\n✅ Total catalog models written: {} → {}",
        report.total_written.to_string().green().bold(),
        report.cache_path.display()
    );

    Ok(())
}
