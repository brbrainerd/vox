//! List models from the on-disk registry cache (`model-catalog.v1.json`).

use anyhow::anyhow;
use clap::Parser;
use vox_orchestrator::models::{Capability, ModelRegistry, ModelSpec};
use vox_orchestrator::route_policy::is_local_http_provider;

/// Default row cap. Chosen to comfortably exceed the current catalog size (~381 models as of
/// 2026-07) so that a plain `vox model list` never silently truncates the output. Explicit
/// `--limit` always wins.
const DEFAULT_LIMIT: usize = 1000;

#[derive(Parser)]
pub struct ListArgs {
    /// Filter by a routing [`Capability`] name (e.g. `supports_tool_use`, `supports_reasoning`).
    #[arg(long)]
    pub capability: Option<String>,
    /// Maximum rows to print.
    #[arg(long, default_value_t = DEFAULT_LIMIT)]
    pub limit: usize,
    /// Only show models runnable on this machine without any cloud credentials
    /// (Ollama / VoxLocal / PopuliMesh providers).
    #[arg(long)]
    pub local_only: bool,
    /// Only show models with `is_free == true` (no per-token cost).
    #[arg(long)]
    pub free_only: bool,
}

/// Default list ordering: local/free models first (so they're never buried past a row limit),
/// then alphabetical by id within each group. This directly addresses the bug where alphabetical
/// sort + a modest default `--limit` silently hid every local Ollama model (their ids sort late,
/// e.g. `qwen3:8b`, `vox-mens-v1:latest`).
fn default_sort_key(m: &ModelSpec) -> (bool, bool, &str) {
    // `false < true`, so negating each "wanted first" predicate makes ascending sort put
    // local providers before cloud ones, and free models before paid ones, within each group.
    (
        !is_local_http_provider(&m.provider_type),
        !m.is_free,
        m.id.as_str(),
    )
}

/// Pure filter/sort/limit core, factored out of [`run`] so the CLI flag wiring (including
/// `--local-only` / `--free-only`) is testable without touching the on-disk registry cache.
fn select_model_ids(
    mut models: Vec<ModelSpec>,
    args: &ListArgs,
    cap: Option<Capability>,
) -> Vec<String> {
    if let Some(c) = cap {
        models.retain(|m| m.capabilities.supports(c));
    }
    if args.local_only {
        models.retain(|m| is_local_http_provider(&m.provider_type));
    }
    if args.free_only {
        models.retain(|m| m.is_free);
    }
    models.sort_by(|a, b| default_sort_key(a).cmp(&default_sort_key(b)));
    models.into_iter().take(args.limit).map(|m| m.id).collect()
}

pub async fn run(args: ListArgs) -> anyhow::Result<()> {
    let cap = match args.capability.as_deref() {
        Some(s) => Some(parse_capability(s)?),
        None => None,
    };
    let reg = ModelRegistry::from_cache();
    let models = reg.list_models();
    for id in select_model_ids(models, &args, cap) {
        println!("{id}");
    }
    Ok(())
}

fn parse_capability(raw: &str) -> anyhow::Result<Capability> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "supports_tool_use" | "tool_use" | "tools" => Ok(Capability::SupportsToolUse),
        "supports_reasoning" | "reasoning" => Ok(Capability::SupportsReasoning),
        "supports_web_search" | "web_search" => Ok(Capability::SupportsWebSearch),
        "supports_image_generation" | "image_generation" => Ok(Capability::SupportsImageGeneration),
        "supports_vision" | "vision" => Ok(Capability::SupportsVision),
        "supports_json" | "json" => Ok(Capability::SupportsJson),
        "supports_audio_input" | "audio_input" => Ok(Capability::SupportsAudioInput),
        "supports_audio_output" | "audio_output" => Ok(Capability::SupportsAudioOutput),
        other => Err(anyhow!(
            "unknown capability {other:?}; try supports_tool_use, supports_reasoning, …"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::models::{PricingSource, ProviderType};

    fn make_spec(id: &str, provider_type: ProviderType, is_free: bool) -> ModelSpec {
        ModelSpec {
            id: id.to_string(),
            canonical_slug: String::new(),
            provider: "test".to_string(),
            provider_type,
            max_tokens: 4096,
            cost_per_1k: if is_free { 0.0 } else { 1.0 },
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: Default::default(),
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    /// Reproduces the original bug scenario: >100 catalog entries, alphabetically sorted, with a
    /// free/local model (`qwen3:8b`) whose id sorts well past position 100. Confirms the new
    /// default sort surfaces it near the top instead of past the default limit.
    #[test]
    fn default_sort_surfaces_local_model_despite_late_alphabetical_id() {
        let mut models: Vec<ModelSpec> = (0..150)
            .map(|i| {
                make_spec(
                    &format!("cloud-model-{i:03}"),
                    ProviderType::OpenRouter,
                    false,
                )
            })
            .collect();
        // "qwen3:8b" sorts after all 150 "cloud-model-*" ids alphabetically.
        models.push(make_spec("qwen3:8b", ProviderType::Ollama, true));
        models.push(make_spec(
            "vox-mens-v1:latest",
            ProviderType::PopuliMesh,
            true,
        ));

        // Old behavior: plain alphabetical sort + limit(100) would drop both local models.
        let mut alpha_sorted = models.clone();
        alpha_sorted.sort_by(|a, b| a.id.cmp(&b.id));
        let old_result: Vec<&str> = alpha_sorted
            .iter()
            .take(100)
            .map(|m| m.id.as_str())
            .collect();
        assert!(
            !old_result.contains(&"qwen3:8b"),
            "sanity check: old alphabetical+limit(100) should reproduce the bug"
        );

        // New behavior: local/free-first sort surfaces them well within any reasonable limit.
        let mut new_sorted = models.clone();
        new_sorted.sort_by(|a, b| default_sort_key(a).cmp(&default_sort_key(b)));
        let new_top: Vec<&str> = new_sorted.iter().take(100).map(|m| m.id.as_str()).collect();
        assert!(
            new_top.contains(&"qwen3:8b"),
            "qwen3:8b must survive the default limit"
        );
        assert!(
            new_top.contains(&"vox-mens-v1:latest"),
            "vox-mens-v1:latest must survive the default limit"
        );

        // And with the new DEFAULT_LIMIT (1000), everything survives regardless of sort order.
        assert!(new_sorted.len() <= DEFAULT_LIMIT);
    }

    #[test]
    fn local_only_flag_filters_to_local_providers() {
        let models = [
            make_spec("cloud-a", ProviderType::OpenRouter, false),
            make_spec("qwen3:8b", ProviderType::Ollama, true),
            make_spec("mesh-model", ProviderType::PopuliMesh, true),
        ];
        let local: Vec<&str> = models
            .iter()
            .filter(|m| is_local_http_provider(&m.provider_type))
            .map(|m| m.id.as_str())
            .collect();
        assert_eq!(local, vec!["qwen3:8b", "mesh-model"]);
    }

    fn default_args() -> ListArgs {
        ListArgs {
            capability: None,
            limit: DEFAULT_LIMIT,
            local_only: false,
            free_only: false,
        }
    }

    /// Drives the actual `--local-only` flag wiring through `select_model_ids` (the same
    /// filter/sort/limit path `run()` calls), rather than testing the predicate in isolation, so
    /// a regression in the `if args.local_only { retain(...) }` wiring itself would be caught.
    #[test]
    fn run_path_local_only_flag_filters_output() {
        let models = vec![
            make_spec("cloud-a", ProviderType::OpenRouter, false),
            make_spec("cloud-b", ProviderType::OpenRouter, true),
            make_spec("qwen3:8b", ProviderType::Ollama, true),
            make_spec("mesh-model", ProviderType::PopuliMesh, true),
        ];

        let mut args = default_args();
        args.local_only = true;
        let ids = select_model_ids(models.clone(), &args, None);
        assert_eq!(ids, vec!["mesh-model".to_string(), "qwen3:8b".to_string()]);

        // Sanity check: with the flag off, cloud models are present too.
        let mut args_off = default_args();
        args_off.local_only = false;
        let ids_off = select_model_ids(models, &args_off, None);
        assert!(ids_off.contains(&"cloud-a".to_string()));
        assert!(ids_off.contains(&"cloud-b".to_string()));
    }

    /// Same treatment for `--free-only`, driven through `select_model_ids`.
    #[test]
    fn run_path_free_only_flag_filters_output() {
        let models = vec![
            make_spec("paid-a", ProviderType::OpenRouter, false),
            make_spec("free-a", ProviderType::OpenRouter, true),
            make_spec("qwen3:8b", ProviderType::Ollama, true),
        ];
        let mut args = default_args();
        args.free_only = true;
        let ids = select_model_ids(models, &args, None);
        assert!(!ids.contains(&"paid-a".to_string()));
        assert!(ids.contains(&"free-a".to_string()));
        assert!(ids.contains(&"qwen3:8b".to_string()));
    }

    /// The `--limit` flag must still be honored exactly when explicitly requested (the fix only
    /// changes the *default*, not the semantics of an explicit limit).
    #[test]
    fn run_path_explicit_limit_is_honored() {
        let models: Vec<ModelSpec> = (0..10)
            .map(|i| make_spec(&format!("model-{i}"), ProviderType::OpenRouter, false))
            .collect();
        let mut args = default_args();
        args.limit = 3;
        let ids = select_model_ids(models, &args, None);
        assert_eq!(ids.len(), 3);
    }
}
