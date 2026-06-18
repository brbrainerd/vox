//! Unified OpenClaw runtime adapter for HTTP and WS gateway operations.

use async_trait::async_trait;
use serde_json::{Value, json};
use thiserror::Error;

use crate::openclaw::{OpenClawClient, OpenClawError, OpenClawRemoteConfig, OpenClawSkillSpec};
use crate::openclaw_discovery::{OpenClawDiscoveryOverrides, resolve_openclaw_endpoints};
use crate::openclaw_gateway_ws::{
    OpenClawGatewayWsClient, OpenClawGatewayWsConfig, OpenClawGatewayWsError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentProvider {
    OpenClaw,
    Hermes,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRuntimeConfig {
    pub provider: AgentProvider,
    pub http_gateway_url: String,
    pub ws_gateway_url: Option<String>,
    pub auth_token: Option<String>,
    pub local_skills_path: Option<std::path::PathBuf>,
}

/// Adapter-level configuration.
#[derive(Debug, Clone)]
pub struct OpenClawAdapterConfig {
    pub http_gateway_url: String,
    pub ws_gateway_url: String,
    pub auth_token: Option<String>,
    pub verify_tls: bool,
}

impl From<OpenClawAdapterConfig> for AgentRuntimeConfig {
    fn from(cfg: OpenClawAdapterConfig) -> Self {
        Self {
            provider: AgentProvider::OpenClaw,
            http_gateway_url: cfg.http_gateway_url,
            ws_gateway_url: Some(cfg.ws_gateway_url),
            auth_token: cfg.auth_token,
            local_skills_path: None,
        }
    }
}

/// Optional overrides when resolving OpenClaw adapter connection settings.
#[derive(Debug, Clone, Default)]
pub struct OpenClawConnectionOverrides {
    pub http_gateway_url: Option<String>,
    pub ws_gateway_url: Option<String>,
    pub well_known_url: Option<String>,
    pub explicit_token: Option<String>,
}

impl OpenClawAdapterConfig {
    pub fn from_env_defaults() -> Self {
        let http_res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenClawUrl);
        let http_gateway_url = http_res
            .expose()
            .unwrap_or("http://127.0.0.1:3000")
            .to_string();

        let ws_res = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOpenClawWsUrl);
        let ws_gateway_url = ws_res
            .expose()
            .unwrap_or("ws://127.0.0.1:18789")
            .to_string();

        let auth_res = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken);
        let auth_token = auth_res.expose().map(str::to_string);

        Self {
            http_gateway_url,
            ws_gateway_url,
            auth_token,
            verify_tls: true,
        }
    }
}

/// Build adapter config using environment defaults with optional explicit token override.
///
/// Token precedence:
/// 1) `explicit_token` when provided and non-empty
/// 2) `VOX_OPENCLAW_TOKEN` from environment
/// 3) no token
#[must_use]
pub fn adapter_config_with_token_override(explicit_token: Option<String>) -> OpenClawAdapterConfig {
    let mut cfg = OpenClawAdapterConfig::from_env_defaults();
    if let Some(token) = explicit_token.and_then(|t| {
        let trimmed = t.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        cfg.auth_token = Some(token);
    }
    cfg
}

/// Resolve OpenClaw adapter config using well-known discovery + precedence rules.
pub async fn resolve_adapter_config(
    overrides: OpenClawConnectionOverrides,
) -> Result<OpenClawAdapterConfig, OpenClawAdapterError> {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides {
        explicit_http_gateway_url: overrides.http_gateway_url.clone(),
        explicit_ws_gateway_url: overrides.ws_gateway_url.clone(),
        explicit_well_known_url: overrides.well_known_url.clone(),
    })
    .await;

    let explicit_token = overrides.explicit_token.and_then(|t| {
        let trimmed = t.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    let auth_token = explicit_token.or_else(|| {
        vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
            .expose()
            .map(str::to_string)
    });

    Ok(OpenClawAdapterConfig {
        http_gateway_url: resolved.http_gateway_url,
        ws_gateway_url: resolved.ws_gateway_url,
        auth_token,
        verify_tls: true,
    })
}

#[derive(Debug, Error)]
pub enum OpenClawAdapterError {
    #[error("OpenClaw HTTP error: {0}")]
    Http(String),
    #[error("OpenClaw WS error: {0}")]
    Ws(String),
    #[error("OpenClaw adapter error: {0}")]
    Other(String),
}

impl From<OpenClawError> for OpenClawAdapterError {
    fn from(value: OpenClawError) -> Self {
        Self::Http(value.to_string())
    }
}

impl From<OpenClawGatewayWsError> for OpenClawAdapterError {
    fn from(value: OpenClawGatewayWsError) -> Self {
        Self::Ws(value.to_string())
    }
}

/// Common OpenClaw/Hermes operation surface used by CLI, runtime, and MCP integrations.
#[async_trait]
pub trait AgentRuntimeAdapter: Send {
    async fn list_remote_skills(&mut self) -> Result<Vec<OpenClawSkillSpec>, OpenClawAdapterError>;
    async fn import_skill(&mut self, slug: &str) -> Result<crate::ArsSkill, OpenClawAdapterError>;
    async fn list_subscriptions(&mut self) -> Result<Value, OpenClawAdapterError>;
    async fn subscribe_domain(&mut self, domain: &str) -> Result<Value, OpenClawAdapterError>;
    async fn unsubscribe_domain(&mut self, domain: &str) -> Result<Value, OpenClawAdapterError>;
    async fn notify_domain(
        &mut self,
        domain: &str,
        message: &str,
    ) -> Result<Value, OpenClawAdapterError>;
    async fn gateway_call(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, OpenClawAdapterError>;
}

pub use AgentRuntimeAdapter as OpenClawRuntimeAdapter;

/// Default adapter implementation that composes existing HTTP skill APIs and WS control plane calls.
pub struct DefaultOpenClawRuntimeAdapter {
    http: OpenClawClient,
    ws: OpenClawGatewayWsClient,
}

impl DefaultOpenClawRuntimeAdapter {
    pub async fn connect(cfg: OpenClawAdapterConfig) -> Result<Self, OpenClawAdapterError> {
        let http = OpenClawClient::new(OpenClawRemoteConfig {
            gateway_url: cfg.http_gateway_url,
            auth_token: cfg.auth_token.clone(),
            verify_tls: cfg.verify_tls,
        })?;
        let ws = OpenClawGatewayWsClient::connect(OpenClawGatewayWsConfig {
            url: cfg.ws_gateway_url,
            token: cfg.auth_token,
            ..OpenClawGatewayWsConfig::default()
        })
        .await?;
        Ok(Self { http, ws })
    }
}

/// Canonical runtime/MCP connection entrypoint for OpenClaw adapter setup.
pub async fn connect_default_runtime_adapter(
    explicit_token: Option<String>,
) -> Result<DefaultOpenClawRuntimeAdapter, OpenClawAdapterError> {
    let cfg = resolve_adapter_config(OpenClawConnectionOverrides {
        explicit_token,
        ..OpenClawConnectionOverrides::default()
    })
    .await?;
    DefaultOpenClawRuntimeAdapter::connect(cfg).await
}

/// Runtime adapter connect entrypoint that supports explicit URL/well-known overrides.
pub async fn connect_runtime_adapter_with_overrides(
    overrides: OpenClawConnectionOverrides,
) -> Result<DefaultOpenClawRuntimeAdapter, OpenClawAdapterError> {
    let cfg = resolve_adapter_config(overrides).await?;
    DefaultOpenClawRuntimeAdapter::connect(cfg).await
}

#[async_trait]
impl AgentRuntimeAdapter for DefaultOpenClawRuntimeAdapter {
    async fn list_remote_skills(&mut self) -> Result<Vec<OpenClawSkillSpec>, OpenClawAdapterError> {
        Ok(self.http.list_skills().await?)
    }

    async fn import_skill(&mut self, slug: &str) -> Result<crate::ArsSkill, OpenClawAdapterError> {
        Ok(self.http.import_skill(slug).await?)
    }

    async fn list_subscriptions(&mut self) -> Result<Value, OpenClawAdapterError> {
        self.ws
            .call_method("subscriptions.list", json!({}))
            .await
            .map_err(Into::into)
    }

    async fn subscribe_domain(&mut self, domain: &str) -> Result<Value, OpenClawAdapterError> {
        self.ws
            .call_method("subscriptions.subscribe", json!({ "domain": domain }))
            .await
            .map_err(Into::into)
    }

    async fn unsubscribe_domain(&mut self, domain: &str) -> Result<Value, OpenClawAdapterError> {
        self.ws
            .call_method("subscriptions.unsubscribe", json!({ "domain": domain }))
            .await
            .map_err(Into::into)
    }

    async fn notify_domain(
        &mut self,
        domain: &str,
        message: &str,
    ) -> Result<Value, OpenClawAdapterError> {
        self.ws
            .call_method(
                "subscriptions.notify",
                json!({ "domain": domain, "description": message }),
            )
            .await
            .map_err(Into::into)
    }

    async fn gateway_call(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Value, OpenClawAdapterError> {
        self.ws
            .call_method(method, params)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests_generic_adaptation {
    use super::*;

    #[test]
    fn test_agent_provider_config_resolution() {
        let cfg = AgentRuntimeConfig {
            provider: AgentProvider::Hermes,
            http_gateway_url: "http://127.0.0.1:8642/v1".to_string(),
            ws_gateway_url: None,
            auth_token: None,
            local_skills_path: Some(std::path::PathBuf::from("~/.hermes/skills")),
        };
        assert_eq!(cfg.provider, AgentProvider::Hermes);
    }
}
