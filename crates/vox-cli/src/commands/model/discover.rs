use clap::Parser;
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use vox_db::{DbConfig, VoxDb};
use vox_orchestrator::catalog::{ModelCatalog, OpenRouterCatalog};
use vox_orchestrator::models::autonomic::{
    DiscoveredModel, DiscoverySource, diff_and_emit_discovery,
};

/// Refresh the model catalog from all sources.
#[derive(Parser)]
pub struct DiscoverArgs {
    /// Force refresh even if cache is warm.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(_args: DiscoverArgs) -> anyhow::Result<()> {
    println!(
        "{} Discovering models...",
        " INFO ".on_blue().white().bold()
    );

    let mut all_models = Vec::new();

    // Load prior snapshot so we can diff and emit DiscoveryEvent for each
    // newly-seen model id (autonomic system L1).
    let cache_dir = vox_config::paths::dot_vox_user_dir().join("cache");
    let cache_file = cache_dir.join("model-catalog.v1.json");
    let prior_ids: HashSet<String> = std::fs::read_to_string(&cache_file)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(&s).ok())
        .map(|v| {
            v.into_iter()
                .filter_map(|x| x.get("id").and_then(|i| i.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 1. OpenRouter
    let or_catalog = OpenRouterCatalog::new();
    if let Ok(models) = or_catalog.refresh().await {
        println!(
            "  ✅ Discovered {} models from OpenRouter",
            models.len().green()
        );
        emit_discovery(DiscoverySource::OpenRouter, &prior_ids, &models);
        all_models.extend(models);
    }

    // 2. Ollama (Local)
    let ollama_url = vox_config::local_ollama_populi_base_url();
    let ollama_catalog = vox_orchestrator::catalog::OllamaCatalog::new(ollama_url);
    if let Ok(models) = ollama_catalog.refresh().await {
        println!(
            "  ✅ Discovered {} models from local Ollama",
            models.len().green()
        );
        all_models.extend(models);
    }

    // 3. Hugging Face
    let hf_catalog = vox_orchestrator::catalog::HuggingFaceCatalog::new();
    if let Ok(models) = hf_catalog.refresh().await {
        println!(
            "  ✅ Discovered {} models from Hugging Face",
            models.len().green()
        );
        all_models.extend(models);
    }

    // 4. Populi Mesh
    let mesh_catalog = vox_orchestrator::catalog::PopuliMeshCatalog::new();
    if let Ok(models) = mesh_catalog.refresh().await {
        println!(
            "  ✅ Discovered {} models from Populi Mesh",
            models.len().green()
        );
        emit_discovery(DiscoverySource::PopuliMesh, &prior_ids, &models);
        all_models.extend(models);
    }

    println!(
        "\n✅ Total discovered models: {}",
        all_models.len().green().bold()
    );

    // Persist a lightweight cache for doctor checks and operational freshness audits.
    let cache_dir = vox_config::paths::dot_vox_user_dir().join("cache");
    std::fs::create_dir_all(&cache_dir)?;
    let cache_file = cache_dir.join("model-catalog.v1.json");
    std::fs::write(&cache_file, serde_json::to_string_pretty(&all_models)?)?;

    // Persist refresh timestamp in user preferences for stale-catalog health checks.
    if let Ok(cfg) = DbConfig::resolve_canonical()
        && let Ok(db) = VoxDb::connect(cfg).await
    {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = db
            .set_user_preference("global", "catalog_refresh", &now_secs.to_string())
            .await;
    }

    Ok(())
}

/// Bridge from `Vec<ModelSpec>` to the autonomic L1 diff. Emits one
/// `DiscoveryEvent` per genuinely-new id (not in `prior` and not retired).
fn emit_discovery(
    source: DiscoverySource,
    prior: &HashSet<String>,
    models: &[vox_orchestrator::models::ModelSpec],
) {
    let discovered: Vec<DiscoveredModel> = models
        .iter()
        .map(|m| DiscoveredModel {
            id: m.id.clone(),
            description: None,
            max_context_tokens: Some(u32::try_from(m.capabilities.max_context).unwrap_or(u32::MAX)),
        })
        .collect();
    let new_ids = diff_and_emit_discovery(source, prior, discovered);
    if !new_ids.is_empty() {
        println!(
            "    {} {} new model id(s) from {}",
            "↳".cyan(),
            new_ids.len().yellow().bold(),
            source.as_str()
        );
    }
}
