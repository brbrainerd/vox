//! `[deploy.coolify]` shapes for Coolify PaaS (parsed from `Vox.toml`).
//!
//! HTTP orchestration lives in `vox-cli`; this module is serde data only.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// How manifest env vars are applied relative to the remote Coolify app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CoolifyEnvReconciliationMode {
    /// Print a diff only (no writes).
    Diff,
    /// Push local env definitions to Coolify (default).
    #[default]
    SyncOnly,
}

/// Either a literal string or a structured env entry (`value` vs `value_env`, flags, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CoolifyEnvVarSpec {
    /// Inline string value for the env key.
    Literal(String),
    /// Structured entry with optional secret-from-env resolution.
    Detailed(CoolifyEnvVarDetail),
}

/// Structured Coolify env var definition under `[deploy.coolify.env]`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoolifyEnvVarDetail {
    /// Inline value (mutually exclusive with `value_env` in validated configs).
    pub value: Option<String>,
    /// Read value from this process environment variable.
    pub value_env: Option<String>,
    /// When true, missing/empty `value_env` is an error; when false, the key is skipped.
    #[serde(default)]
    pub required: bool,
    pub is_preview: Option<bool>,
    pub is_literal: Option<bool>,
    pub is_multiline: Option<bool>,
    pub is_shown_once: Option<bool>,
}

fn default_token_env() -> String {
    "COOLIFY_TOKEN".to_string()
}

/// Coolify application and sync settings (`[deploy.coolify]`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoolifyDeployConfig {
    pub base_url: Option<String>,
    pub base_url_env: Option<String>,
    pub app_uuid: Option<String>,
    pub app_uuid_env: Option<String>,
    pub webhook_url: Option<String>,
    pub webhook_url_env: Option<String>,
    #[serde(default = "default_token_env")]
    pub token_env: String,
    pub branch: Option<String>,
    #[serde(default)]
    pub force_rebuild: bool,
    #[serde(default)]
    pub env: BTreeMap<String, CoolifyEnvVarSpec>,
    pub env_reconciliation_mode: Option<CoolifyEnvReconciliationMode>,
    #[serde(default)]
    pub health_endpoints: Vec<String>,
    /// Poll interval when waiting on deployment status (seconds). CLI defaults to 5 if unset.
    pub poll_interval_secs: Option<u64>,
    /// Max time to poll deployment status (seconds). CLI defaults to 600 if unset.
    pub poll_timeout_secs: Option<u64>,
    /// Optional env var name whose value is shown in rollback runbooks.
    pub rollback_revision_env: Option<String>,
}

impl Default for CoolifyDeployConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            base_url_env: None,
            app_uuid: None,
            app_uuid_env: None,
            webhook_url: None,
            webhook_url_env: None,
            token_env: default_token_env(),
            branch: None,
            force_rebuild: false,
            env: BTreeMap::new(),
            env_reconciliation_mode: None,
            health_endpoints: Vec::new(),
            poll_interval_secs: None,
            poll_timeout_secs: None,
            rollback_revision_env: None,
        }
    }
}

#[cfg(test)]
mod semcov_wave3_tests {
    #![allow(unused_imports)]
    use super::*;
    use crate::deploy_coolify::CoolifyDeployConfig;

    #[test]
    fn coolify_deploy_config_default_sets_token_env() {
        let cfg = CoolifyDeployConfig::default();
        assert_eq!(cfg.token_env, "COOLIFY_TOKEN");
        assert!(cfg.base_url.is_none());
        assert!(cfg.app_uuid.is_none());
        assert!(!cfg.force_rebuild);
        assert!(cfg.env.is_empty());
        assert!(cfg.env_reconciliation_mode.is_none());
        assert!(cfg.health_endpoints.is_empty());
        assert!(cfg.poll_interval_secs.is_none());
        assert!(cfg.poll_timeout_secs.is_none());
    }
}
