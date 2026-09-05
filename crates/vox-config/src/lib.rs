//! Centralized configuration for Vox: env vars, defaults, and path resolution.
//!
//! Precedence: CLI args > env > config file > defaults.

/// Re-export of the LLM/AI setting-key SSOT. `vox-config` is a *view* over
/// [`vox_llm_config`]: typed env-resolving accessors here are backed by the registry.
pub use vox_llm_config;

pub mod bootstrap_inference;
pub mod config;
pub mod config_field;
pub mod config_key;
pub mod config_registry;
pub mod config_watch;
pub mod env_parse;
pub mod graphify;
pub mod inference;
pub mod model_routing;
pub mod operator_registry;
pub mod paths;
pub mod policy;
pub mod project_manifest;
#[cfg(feature = "llm-egress")]
pub mod resolve_egress;
pub mod rollout;
pub mod routing_migration;
pub mod routing_policy;
pub mod scholarly;
pub mod secrets;
pub mod serde_defaults;
pub mod snapshot;
pub mod timeouts;
pub mod toml_config;

pub use bootstrap_inference::{
    NLI_FALLBACK, OPENROUTER_AUTO, OPENROUTER_FREE, OPENROUTER_FREE_FALLBACK_MODELS,
    RESEARCH_FLASH_FALLBACK, REVIEW_PREMIUM_FALLBACK,
};
pub use config::{BuildTarget, GamifyMode, VoxConfig, WebRunMode};
pub use config_field::ConfigField;
pub use vox_config_derive::VoxConfig;

/// Implemented by every `#[derive(VoxConfig)]` struct. The `vox-cli` aggregator
/// collects `config_keys()` across domains; `catalog()` feeds the GUI.
pub trait VoxConfigDomain: Sized {
    fn merge_env(&mut self);
    fn config_keys() -> &'static [crate::config_key::ConfigKey];
    fn catalog(&self) -> Vec<ConfigField>;
}
pub use config_watch::{ConfigSnapshot, ConfigWatch};
pub use graphify::{
    CORPORA_REL_PATH, CorpusStatus, GraphifyCorporaRegistry, GraphifyCorpus, GraphifyError,
    GraphifyKnowledgeNode, GraphifyManifest, LEGACY_GRAPHIFY_OUT_DIR, LexicalGraphHit,
    MANIFEST_BASENAME, assess_corpus_status, graph_stats_from_json, lexical_search_graph,
    load_graphify_corpora, project_graph_nodes_for_ingest,
};
pub use inference::{
    InferenceProfile, LOCAL_OLLAMA_POPULI_BASE_URL_DEFAULT, OPENAI_CHAT_COMPLETIONS_URL,
    OPENAI_EMBEDDINGS_URL, OPENROUTER_CHAT_COMPLETIONS_URL, OPENROUTER_EMBEDDINGS_URL,
    OPENROUTER_MODELS_LIST_URL, anthropic_tuning_temperature, anthropic_tuning_top_p,
    gemini_tuning_temperature, gemini_tuning_top_p, hf_chat_model_preference,
    hf_dedicated_chat_completions_url, hf_dedicated_chat_model, hf_router_chat_completions_url,
    huggingface_hub_token, inference_profile_allows_local_ollama_http, inference_profile_from_env,
    local_ollama_populi_base_url, ollama_tuning_num_ctx, ollama_tuning_temperature,
    ollama_tuning_top_p, openai_chat_completions_url, openai_compatible_base_url,
    openai_embeddings_url, openai_tuning_temperature, openai_tuning_top_p, openrouter_api_key,
    openrouter_base_url, openrouter_chat_completions_url, openrouter_chat_model_preference,
    openrouter_embeddings_url, openrouter_models_list_url, sanitize_chatml,
    together_tuning_temperature, together_tuning_top_p,
};
pub use model_routing::{
    ClassifierPinConfig, ExplorationConfig, LatencyBands, ModelPinsConfig, ModelRoutingConfig,
    PromotionThresholds, SafetyConfig, load_model_pins_config, load_model_routing_config,
};
pub use paths::{
    APP_DIR_NAME, DEFAULT_DB_FILENAME, MCP_SESSIONS_DIR_BASENAME, config_dir, data_dir,
    default_db_path, dot_vox_user_dir, local_user_id, mcp_sessions_dir, repo_backend_artifact_dir,
    repo_memory_cache_dir, repo_tooling_cache_dir, script_cache_dir, state_dir, user_home_dir,
};
pub use policy::hitl_policy::HitlPolicy;
pub use policy::overrides;
pub use policy::registry::{
    PolicyDomain, PolicyEntry, PolicyRegistry, PolicyRegistryError, PolicySeverity, PolicySource,
    PolicySourceKind, REGISTRY_REL_PATH, load_policy_registry,
};
pub use policy::status::{
    Hit, PolicyResult, PolicyRunReport, PolicyStatusError, RunStatus, STATUS_DIR_REL, load_status,
    load_status_for_branches, sanitize_branch, status_path,
};
pub use project_manifest::{
    BundleAssetsToml, BundleTomlFragment, ProjectManifest, WorkspaceTomlFragment,
};
pub use rollout::{
    RolloutFlagSnapshot, db_circuit_breaker_env_enabled,
    db_embedded_replica_integration_gate_armed, db_sync_remote_integration_gate_armed, env_truthy,
    orchestration_lineage_persist_enabled, rollout_flag_snapshot,
    workflow_journal_codex_persist_enabled,
};
pub use routing_migration::{
    secrets_cutover_blocks_legacy_env, secrets_cutover_blocks_legacy_env_raw,
    trace_openrouter_chat_env_migration_once,
};
pub use routing_policy::{
    AutoModelStrategy, AutoRoutingPriority, GeminiRoutePolicy, GeminiRouteTargets,
    OpenRouterRouteHint, RouteCostPreference, derive_openrouter_route_hint,
    gemini_route_targets_from_env, resolve_openrouter_model,
};
pub use secrets::secrets_str;

/// Minimum Vox MCP server version required for full agent capability.
pub const VOX_MCP_MIN_VERSION: &str = ">=0.2.0";

/// URL path prefix for serving stored files.
pub const STORAGE_URL_PREFIX: &str = "/storage";

/// Maximum number of WAL entries to buffer before forcing a flush to disk.
pub const WAL_FLUSH_BATCH_SIZE: usize = 32;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_version() {
        assert_eq!(VOX_MCP_MIN_VERSION, ">=0.2.0");
    }

    #[test]
    fn test_path_constants() {
        assert_eq!(APP_DIR_NAME, "vox");
        assert_eq!(DEFAULT_DB_FILENAME, "vox.db");
    }

    #[test]
    fn inference_local_url_is_http_base() {
        let s = inference::local_ollama_populi_base_url();
        assert!(
            s.starts_with("http://") || s.starts_with("https://"),
            "expected URL scheme: {s}"
        );
    }

    #[test]
    fn inference_profile_default_is_desktop_ollama() {
        assert_eq!(InferenceProfile::default(), InferenceProfile::DesktopOllama);
    }

    #[test]
    fn inference_profile_ollama_http_gate() {
        assert!(InferenceProfile::DesktopOllama.allows_local_ollama_http());
        assert!(InferenceProfile::LanGateway.allows_local_ollama_http());
        assert!(!InferenceProfile::CloudOpenAiCompatible.allows_local_ollama_http());
        assert!(!InferenceProfile::MobileLitert.allows_local_ollama_http());
        assert!(!InferenceProfile::MobileCoreml.allows_local_ollama_http());
    }

    #[test]
    fn routing_migration_cutover_raw_parses_phases() {
        assert!(routing_migration::secrets_cutover_blocks_legacy_env_raw(
            "enforce"
        ));
        assert!(routing_migration::secrets_cutover_blocks_legacy_env_raw(
            "  Decommission \n"
        ));
        assert!(!routing_migration::secrets_cutover_blocks_legacy_env_raw(
            "shadow"
        ));
        assert!(!routing_migration::secrets_cutover_blocks_legacy_env_raw(
            ""
        ));
    }
}
