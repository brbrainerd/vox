//! `VoxConfig` struct and defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::gamify_web::{BuildTarget, GamifyMode, WebRunMode};
use crate::policy::hitl_policy::HitlPolicy;

/// Full Vox toolchain configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoxConfig {
    pub model: String,
    pub openrouter_key: Option<String>,
    pub openai_key: Option<String>,
    pub gemini_key: Option<String>,
    pub anthropic_key: Option<String>,
    pub daily_budget_usd: f64,
    pub per_session_budget_usd: f64,
    pub data_dir: PathBuf,
    pub model_dir: PathBuf,
    pub train_epochs: usize,
    pub train_batch_size: usize,
    pub mcp_binary: Option<PathBuf>,
    pub db_url: Option<String>,
    pub gamify_enabled: bool,
    pub gamify_mode: GamifyMode,
    pub web_run_mode: WebRunMode,
    pub web_tanstack_start: bool,
    pub build_target: BuildTarget,
    pub hitl: HitlPolicy,
    /// Global ceiling on concurrent LLM HTTP requests across all providers.
    pub llm_max_concurrent_requests: usize,
    /// Per-provider override for OpenRouter (None = use the global ceiling).
    pub llm_openrouter_max_concurrent: Option<usize>,
    /// Per-provider override for OpenAI (None = use the global ceiling).
    pub llm_openai_max_concurrent: Option<usize>,
    /// Max retry attempts on a 429 before surfacing the error.
    pub llm_retry_max_attempts: u32,
}

impl Default for VoxConfig {
    fn default() -> Self {
        Self {
            model: "anthropic/claude-sonnet-4".to_string(),
            openrouter_key: None,
            openai_key: None,
            gemini_key: None,
            anthropic_key: None,
            daily_budget_usd: 5.0,
            per_session_budget_usd: 1.0,
            data_dir: PathBuf::from("target/dogfood"),
            model_dir: crate::paths::data_dir()
                .map(|d| d.join("models"))
                .unwrap_or_else(|| PathBuf::from(crate::paths::REPO_MODELS_DIR)),
            train_epochs: 3,
            train_batch_size: 256,
            mcp_binary: None,
            db_url: None,
            gamify_enabled: true,
            gamify_mode: GamifyMode::default(),
            web_run_mode: WebRunMode::default(),
            web_tanstack_start: false,
            build_target: BuildTarget::default(),
            hitl: HitlPolicy::default(),
            llm_max_concurrent_requests: 8,
            llm_openrouter_max_concurrent: None,
            llm_openai_max_concurrent: None,
            llm_retry_max_attempts: 4,
        }
    }
}
