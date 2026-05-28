//! Background catalog refresh loop.
//!
//! Runs as a long-lived `tokio::spawn` task launched from
//! [`Orchestrator::spawn_background_tasks`]. Every `REFRESH_INTERVAL_SECS` (default 6 h,
//! ±20 min jitter) it fetches the OpenRouter model catalog and the LiteLLM pricing oracle,
//! merges the results into the live `ModelRegistry`, and persists the result to
//! `~/.vox/cache/model-catalog.v1.json` so the next cold-start loads fresh data.
//!
//! The loop is entirely opt-out: if either upstream fetch fails the registry keeps whatever
//! pricing it already has and the loop simply logs a warning and sleeps until the next cycle.

use std::sync::Arc;
use std::time::Duration;

use crate::catalog::{AnthropicDirectCatalog, LiteLLMCatalog, ModelCatalog, OpenRouterCatalog};
use crate::models::spec::PricingSource;
use crate::orchestrator::Orchestrator;

/// Default interval between catalog refreshes (6 hours).
const REFRESH_INTERVAL_SECS: u64 = 6 * 3_600;

/// Maximum random jitter added to the sleep interval (±20 minutes).
/// Avoids thundering-herd on multi-process or multi-instance deployments.
const JITTER_MAX_SECS: u64 = 1_200;

/// Entry point for the background catalog refresh task.
///
/// Sleeps for the first interval before the first refresh — the startup path
/// (`ModelRegistry::new` → `maybe_refresh_catalogs`) handles the initial load.
pub async fn run_catalog_refresh_loop(orch: Arc<Orchestrator>) {
    loop {
        let interval = REFRESH_INTERVAL_SECS + jitter_secs(JITTER_MAX_SECS);
        tracing::debug!(
            target: "vox.orchestrator.catalog_refresh",
            interval_secs = interval,
            "catalog refresh loop sleeping"
        );
        tokio::time::sleep(Duration::from_secs(interval)).await;

        // Honour the stop flag so the loop exits cleanly on shutdown.
        if orch.stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!(target: "vox.orchestrator.catalog_refresh", "stop flag set; exiting refresh loop");
            break;
        }

        refresh_once(&orch).await;
    }
}

async fn refresh_once(orch: &Arc<Orchestrator>) {
    // ── 1. OpenRouter ─────────────────────────────────────────────────────────
    let mut openrouter_models = match OpenRouterCatalog::new().refresh().await {
        Ok(m) => {
            tracing::debug!(
                target: "vox.orchestrator.catalog_refresh",
                count = m.len(),
                "openrouter catalog fetched"
            );
            m
        }
        Err(e) => {
            tracing::warn!(
                target: "vox.orchestrator.catalog_refresh",
                error = %e,
                "openrouter catalog refresh failed; keeping existing pricing"
            );
            vec![]
        }
    };

    // Mark all freshly-fetched OpenRouter models with the correct source.
    for m in &mut openrouter_models {
        if m.pricing_source == PricingSource::Bootstrap {
            m.pricing_source = PricingSource::OpenRouter;
        }
    }

    // ── 2. LiteLLM pricing oracle ─────────────────────────────────────────────
    let litellm_entries = match LiteLLMCatalog::new().fetch().await {
        Ok(entries) => {
            tracing::debug!(
                target: "vox.orchestrator.catalog_refresh",
                count = entries.len(),
                "litellm pricing oracle fetched"
            );
            entries
        }
        Err(e) => {
            tracing::warn!(
                target: "vox.orchestrator.catalog_refresh",
                error = %e,
                "litellm pricing fetch failed; cache costs will not be updated this cycle"
            );
            std::collections::HashMap::new()
        }
    };

    // ── 3. Anthropic direct catalog (key-gated; discovers new models before OpenRouter lists them) ──
    let anthropic_models: Vec<crate::models::ModelSpec> =
        match AnthropicDirectCatalog::new().refresh().await {
            Ok(mut models) => {
                for m in &mut models {
                    if m.pricing_source == PricingSource::Bootstrap {
                        m.pricing_source = PricingSource::AnthropicDirect;
                    }
                }
                tracing::debug!(
                    target: "vox.orchestrator.catalog_refresh",
                    count = models.len(),
                    "anthropic direct catalog fetched"
                );
                models
            }
            Err(_) => {
                // No key set or request failed — silent skip (expected in most deployments).
                vec![]
            }
        };

    if openrouter_models.is_empty() && litellm_entries.is_empty() && anthropic_models.is_empty() {
        tracing::debug!(
            target: "vox.orchestrator.catalog_refresh",
            "all sources empty; skipping registry update"
        );
        return;
    }

    crate::catalog_classifier::classify_models(&mut openrouter_models).await;

    // ── 4. Apply to registry under write lock ─────────────────────────────────
    let total_registered = {
        let mut registry = orch.models.write().unwrap();
        let count = openrouter_models.len() + anthropic_models.len();
        for m in openrouter_models {
            registry.register(m);
        }
        for m in anthropic_models {
            // Only register if the model isn't already in the registry from OpenRouter.
            if registry.get(&m.id).is_none() {
                registry.register(m);
            }
        }
        if !litellm_entries.is_empty() {
            registry.apply_litellm_pricing(&litellm_entries);
        }
        count
    };

    tracing::info!(
        target: "vox.orchestrator.catalog_refresh",
        total_models = total_registered,
        litellm_entries = litellm_entries.len(),
        "background catalog refresh applied"
    );

    // ── 5. Run Admission Filter & Persist updated catalog ─────────────────────
    let mut snapshot: Vec<crate::models::ModelSpec> = {
        let registry = orch.models.read().unwrap();
        registry.list_models()
    };

    if let Some(db) = orch.db() {
        match crate::models::admission::ModelAdmissionFilter::promote_calibrated_models(
            &db,
            &mut snapshot,
        )
        .await
        {
            Ok(count) if count > 0 => {
                tracing::info!(target: "vox.orchestrator.catalog_refresh", promoted = count, "models promoted to telemetry status");
                // Re-register promoted models back into the active registry
                let mut registry = orch.models.write().unwrap();
                for m in &snapshot {
                    if m.pricing_source == crate::models::spec::PricingSource::Telemetry {
                        registry.register(m.clone());
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: "vox.orchestrator.catalog_refresh", error = %e, "model admission filter failed");
            }
            _ => {}
        }
    }

    if let Ok(json) = serde_json::to_string(&snapshot) {
        let cache_file = vox_config::paths::dot_vox_user_dir()
            .join("cache")
            .join("model-catalog.v1.json");
        if let Some(parent) = cache_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&cache_file, &json) {
            tracing::warn!(
                target: "vox.orchestrator.catalog_refresh",
                error = %e,
                "failed to persist catalog cache"
            );
        } else {
            tracing::debug!(
                target: "vox.orchestrator.catalog_refresh",
                models = snapshot.len(),
                "catalog cache written"
            );
        }
    }
}

/// Summary returned by a foreground catalog refresh (used by the CLI `pricing refresh` command).
pub struct RefreshReport {
    /// Number of models fetched from OpenRouter.
    pub openrouter_count: usize,
    /// Number of pricing entries fetched from LiteLLM.
    pub litellm_count: usize,
    /// Number of models fetched from the Anthropic direct API (0 if no key is set).
    pub anthropic_count: usize,
    /// Total models written to the cache file.
    pub total_written: usize,
    /// Path written to.
    pub cache_path: std::path::PathBuf,
}

/// Unified discover/refresh report (CLI `vox model discover` + background parity).
pub struct UnifiedCatalogReport {
    pub openrouter_count: usize,
    pub ollama_count: usize,
    pub huggingface_count: usize,
    pub mesh_count: usize,
    pub mens_count: usize,
    pub litellm_count: usize,
    pub anthropic_count: usize,
    pub total_written: usize,
    pub cache_path: std::path::PathBuf,
    pub new_discovery_ids: Vec<String>,
}

pub const MODEL_CATALOG_LAST_REFRESH_KEY: &str = "model_catalog_last_refresh";

async fn persist_catalog_refresh_timestamp() {
    use std::time::{SystemTime, UNIX_EPOCH};
    if let Ok(cfg) = vox_db::DbConfig::resolve_canonical()
        && let Ok(db) = vox_db::VoxDb::connect(cfg).await
    {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = db
            .set_user_preference("global", MODEL_CATALOG_LAST_REFRESH_KEY, &now_secs.to_string())
            .await;
    }
}

async fn register_supplemental_catalogs(
    registry: &mut crate::models::ModelRegistry,
) -> (usize, usize, usize, usize) {
    use crate::catalog::{
        HuggingFaceCatalog, MensCatalog, ModelCatalog, OllamaCatalog, PopuliMeshCatalog,
    };

    let mut ollama_count = 0usize;
    let ollama_url = vox_config::local_ollama_populi_base_url();
    if let Ok(models) = OllamaCatalog::new(ollama_url).refresh().await {
        ollama_count = models.len();
        for m in models {
            registry.register(m);
        }
    }

    let mut hf_count = 0usize;
    if let Ok(models) = HuggingFaceCatalog::new().refresh().await {
        hf_count = models.len();
        for m in models {
            registry.register(m);
        }
    }

    let mut mesh_count = 0usize;
    if let Ok(models) = PopuliMeshCatalog::new().refresh().await {
        mesh_count = models.len();
        for m in models {
            registry.register(m);
        }
    }

    let mut mens_count = 0usize;
    let repo_root = vox_repository::find_project_manifest_root(
        &std::env::current_dir().unwrap_or_default(),
    )
    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    if let Ok(models) = MensCatalog::new(&repo_root).refresh().await {
        mens_count = models.len();
        for m in models {
            registry.register(m);
        }
    }

    (ollama_count, hf_count, mesh_count, mens_count)
}

/// Single refresh pipeline for CLI discover + foreground pricing refresh parity.
pub async fn run_unified_catalog_refresh(_force: bool) -> anyhow::Result<UnifiedCatalogReport> {
    use crate::models::ModelRegistry;
    use crate::models::autonomic::{DiscoveredModel, DiscoverySource, diff_and_emit_discovery};
    use std::collections::HashSet;

    let cache_file = vox_config::paths::dot_vox_user_dir()
        .join("cache")
        .join("model-catalog.v1.json");
    let prior_ids: HashSet<String> = std::fs::read_to_string(&cache_file)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<crate::models::ModelSpec>>(&s).ok())
        .map(|v| v.into_iter().map(|m| m.id).collect())
        .unwrap_or_default();

    let mut registry = ModelRegistry::from_cache();
    let mut new_discovery_ids = Vec::new();

    let mut openrouter_count = 0usize;
    match OpenRouterCatalog::new().refresh().await {
        Ok(mut models) => {
            for m in &mut models {
                if m.pricing_source == PricingSource::Bootstrap {
                    m.pricing_source = PricingSource::OpenRouter;
                }
            }
            openrouter_count = models.len();
            let discovered: Vec<DiscoveredModel> = models
                .iter()
                .map(|m| DiscoveredModel {
                    id: m.id.clone(),
                    description: None,
                    max_context_tokens: Some(
                        u32::try_from(m.capabilities.max_context).unwrap_or(u32::MAX),
                    ),
                })
                .collect();
            new_discovery_ids.extend(diff_and_emit_discovery(
                DiscoverySource::OpenRouter,
                &prior_ids,
                discovered,
            ));
            for m in models {
                registry.register(m);
            }
        }
        Err(e) => {
            eprintln!("warn: OpenRouter fetch failed ({e}); keeping existing models");
        }
    }

    let litellm_count;
    match LiteLLMCatalog::new().fetch().await {
        Ok(entries) => {
            litellm_count = entries.len();
            registry.apply_litellm_pricing(&entries);
        }
        Err(e) => {
            litellm_count = 0;
            eprintln!("warn: LiteLLM fetch failed ({e}); cache costs not updated");
        }
    }

    let mut anthropic_count = 0usize;
    if let Ok(mut models) = AnthropicDirectCatalog::new().refresh().await {
        for m in &mut models {
            if m.pricing_source == PricingSource::Bootstrap {
                m.pricing_source = PricingSource::AnthropicDirect;
            }
        }
        anthropic_count = models.len();
        for m in models {
            if registry.get(&m.id).is_none() {
                registry.register(m);
            }
        }
    }

    let (ollama_count, huggingface_count, mesh_count, mens_count) =
        register_supplemental_catalogs(&mut registry).await;

    if mesh_count > 0 {
        let mesh_models = registry
            .list_models()
            .into_iter()
            .filter(|m| m.provider_type == crate::models::ProviderType::PopuliMesh)
            .collect::<Vec<_>>();
        let discovered: Vec<DiscoveredModel> = mesh_models
            .iter()
            .map(|m| DiscoveredModel {
                id: m.id.clone(),
                description: None,
                max_context_tokens: Some(
                    u32::try_from(m.capabilities.max_context).unwrap_or(u32::MAX),
                ),
            })
            .collect();
        new_discovery_ids.extend(diff_and_emit_discovery(
            DiscoverySource::PopuliMesh,
            &prior_ids,
            discovered,
        ));
    }

    let mut snapshot = registry.list_models();
    crate::catalog_classifier::classify_models(&mut snapshot).await;

    if let Ok(db) = vox_db::VoxDb::open_default().await {
        if let Ok(count) =
            crate::models::admission::ModelAdmissionFilter::promote_calibrated_models(
                &db,
                &mut snapshot,
            )
            .await
        {
            if count > 0 {
                for m in &snapshot {
                    if m.pricing_source == PricingSource::Telemetry {
                        registry.register(m.clone());
                    }
                }
            }
        }
    }

    let total_written = snapshot.len();
    if let Some(parent) = cache_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&cache_file, serde_json::to_string_pretty(&snapshot)?)?;
    persist_catalog_refresh_timestamp().await;

    Ok(UnifiedCatalogReport {
        openrouter_count,
        ollama_count,
        huggingface_count,
        mesh_count,
        mens_count,
        litellm_count,
        anthropic_count,
        total_written,
        cache_path: cache_file,
        new_discovery_ids,
    })
}

/// Foreground catalog refresh for the CLI — no running daemon required.
pub async fn run_foreground_refresh() -> anyhow::Result<RefreshReport> {
    let unified = run_unified_catalog_refresh(true).await?;
    Ok(RefreshReport {
        openrouter_count: unified.openrouter_count,
        litellm_count: unified.litellm_count,
        anthropic_count: unified.anthropic_count,
        total_written: unified.total_written,
        cache_path: unified.cache_path,
    })
}

/// Cheap deterministic jitter derived from the current time's sub-second nanos.
fn jitter_secs(max_secs: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    nanos % (max_secs + 1)
}

