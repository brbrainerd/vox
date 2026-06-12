//! Internal TOML deserialize shapes for `Vox.toml` / `~/.vox/config.toml`.

use std::path::PathBuf;

use serde::Deserialize;

use super::gamify_web::{BuildTarget, GamifyMode, WebRunMode};

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct VoxToml {
    pub(super) vox: Option<VoxTomlSection>,
    pub(super) train: Option<TrainTomlSection>,
    pub(super) db: Option<DbTomlSection>,
    pub(super) web: Option<WebTomlSection>,
    pub(super) build: Option<BuildTomlSection>,
    pub(super) llm: Option<LlmTomlSection>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct BuildTomlSection {
    pub(super) target: Option<BuildTarget>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct VoxTomlSection {
    pub(super) model: Option<String>,
    pub(super) daily_budget_usd: Option<f64>,
    pub(super) per_session_budget_usd: Option<f64>,
    pub(super) mcp_binary: Option<PathBuf>,
    pub(super) gamify_enabled: Option<bool>,
    pub(super) gamify_mode: Option<GamifyMode>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct TrainTomlSection {
    pub(super) data_dir: Option<PathBuf>,
    pub(super) model_dir: Option<PathBuf>,
    pub(super) epochs: Option<usize>,
    pub(super) batch_size: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct DbTomlSection {
    pub(super) url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct WebTomlSection {
    pub(super) run_mode: Option<WebRunMode>,
    pub(super) tanstack_start: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(super) struct LlmTomlSection {
    pub(super) max_concurrent_requests: Option<usize>,
    pub(super) openrouter_max_concurrent: Option<usize>,
    pub(super) openai_max_concurrent: Option<usize>,
    pub(super) retry_max_attempts: Option<u32>,
}
