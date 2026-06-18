//! Agent MCP tools backed by the native AgentRuntimeAdapter.

use crate::params::{
    OpenClawDomainParams, OpenClawGatewayCallParams, OpenClawNotifyParams, ToolResult,
};
use crate::server_state::ServerState;
use vox_openclaw_runtime::{
    AgentRuntimeAdapter, DefaultHermesRuntimeAdapter, connect_default_runtime_adapter,
};

type BoxedAgentAdapter = Box<dyn AgentRuntimeAdapter + Send>;

#[cfg(not(test))]
type SharedAgentAdapter = std::sync::Arc<tokio::sync::Mutex<BoxedAgentAdapter>>;

#[cfg(not(test))]
static AGENT_ADAPTER: tokio::sync::OnceCell<SharedAgentAdapter> =
    tokio::sync::OnceCell::const_new();

async fn connect_agent_adapter_uncached() -> Result<BoxedAgentAdapter, String> {
    #[cfg(test)]
    if let Some(result) = test_hook::connect_if_configured().await {
        return result;
    }

    let config = vox_config::VoxConfig::load();
    let provider_str = config.agent_provider.to_lowercase();
    if provider_str == "hermes" {
        let http_url = std::env::var("VOX_HERMES_URL")
            .unwrap_or_else(|_| "http://localhost:8642/v1".to_string());
        let auth_token = std::env::var("VOX_HERMES_TOKEN").ok().or_else(|| {
            vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
                .expose()
                .map(|s| s.to_string())
        });
        let local_skills_path = std::env::var("VOX_HERMES_SKILLS_PATH")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| Some(std::path::PathBuf::from(".agents/skills")));
        let adapter_config = vox_openclaw_runtime::AgentRuntimeConfig {
            provider: vox_openclaw_runtime::AgentProvider::Hermes,
            http_gateway_url: http_url,
            ws_gateway_url: None,
            auth_token,
            local_skills_path,
        };
        Ok(Box::new(DefaultHermesRuntimeAdapter::new(adapter_config)) as BoxedAgentAdapter)
    } else {
        let secrets_token = vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
            .expose()
            .map(|s| s.to_string());
        connect_default_runtime_adapter(secrets_token)
            .await
            .map(|adapter| Box::new(adapter) as BoxedAgentAdapter)
            .map_err(|e| format!("agent adapter connect failed: {e}"))
    }
}

/// List remote/local agent skills.
pub async fn agent_list_remote(_state: &ServerState) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.list_remote_skills().await {
                Ok(skills) => ToolResult::ok(serde_json::json!({ "skills": skills })).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.list_remote_skills().await {
                    Ok(skills) => ToolResult::ok(serde_json::json!({ "skills": skills })).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

/// Generic agent gateway method call.
pub async fn agent_gateway_call(_state: &ServerState, params: OpenClawGatewayCallParams) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.gateway_call(&params.method, params.params).await {
                Ok(payload) => ToolResult::ok(payload).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.gateway_call(&params.method, params.params).await {
                    Ok(payload) => ToolResult::ok(payload).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

/// List agent gateway subscriptions for the current session.
pub async fn agent_subscriptions(_state: &ServerState) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.list_subscriptions().await {
                Ok(payload) => ToolResult::ok(payload).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.list_subscriptions().await {
                    Ok(payload) => ToolResult::ok(payload).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

/// Subscribe this session to a gateway domain.
pub async fn agent_subscribe(_state: &ServerState, params: OpenClawDomainParams) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.subscribe_domain(&params.domain).await {
                Ok(payload) => ToolResult::ok(payload).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.subscribe_domain(&params.domain).await {
                    Ok(payload) => ToolResult::ok(payload).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

/// Unsubscribe this session from a gateway domain.
pub async fn agent_unsubscribe(_state: &ServerState, params: OpenClawDomainParams) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.unsubscribe_domain(&params.domain).await {
                Ok(payload) => ToolResult::ok(payload).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.unsubscribe_domain(&params.domain).await {
                    Ok(payload) => ToolResult::ok(payload).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

/// Notify a domain with a message payload.
pub async fn agent_notify(_state: &ServerState, params: OpenClawNotifyParams) -> String {
    #[cfg(test)]
    {
        match connect_agent_adapter_uncached().await {
            Ok(mut adapter) => match adapter.notify_domain(&params.domain, &params.message).await {
                Ok(payload) => ToolResult::ok(payload).to_json(),
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            },
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }

    #[cfg(not(test))]
    {
        match shared_adapter().await {
            Ok(adapter) => {
                let mut adapter = adapter.lock().await;
                match adapter.notify_domain(&params.domain, &params.message).await {
                    Ok(payload) => ToolResult::ok(payload).to_json(),
                    Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
                }
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
        }
    }
}

#[cfg(not(test))]
async fn shared_adapter() -> Result<SharedAgentAdapter, String> {
    AGENT_ADAPTER
        .get_or_try_init(|| async {
            let adapter = connect_agent_adapter_uncached().await?;
            Ok(std::sync::Arc::new(tokio::sync::Mutex::new(adapter)))
        })
        .await
        .map(std::sync::Arc::clone)
}

#[cfg(test)]
mod test_hook {
    use super::BoxedAgentAdapter;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, OnceLock};

    type ConnectFuture = Pin<Box<dyn Future<Output = Result<BoxedAgentAdapter, String>> + Send>>;
    type ConnectHook = dyn Fn() -> ConnectFuture + Send + Sync + 'static;

    static CONNECT_HOOK: OnceLock<Mutex<Option<Arc<ConnectHook>>>> = OnceLock::new();

    pub(super) fn set_connect_hook(hook: Arc<ConnectHook>) {
        let cell = CONNECT_HOOK.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = cell.lock() {
            *guard = Some(hook);
        }
    }

    pub(super) fn clear_connect_hook() {
        let cell = CONNECT_HOOK.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = cell.lock() {
            *guard = None;
        }
    }

    pub(super) async fn connect_if_configured() -> Option<Result<BoxedAgentAdapter, String>> {
        let cell = CONNECT_HOOK.get_or_init(|| Mutex::new(None));
        let hook = match cell.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };
        match hook {
            Some(h) => Some(h().await),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serial_test::serial;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use vox_openclaw_runtime::{OpenClawAdapterError, OpenClawSkillSpec};

    struct MockAdapter;

    #[async_trait]
    impl AgentRuntimeAdapter for MockAdapter {
        async fn list_remote_skills(
            &mut self,
        ) -> Result<Vec<OpenClawSkillSpec>, OpenClawAdapterError> {
            Ok(vec![OpenClawSkillSpec {
                name: "mock-skill".to_string(),
                version: "1.0.0".to_string(),
                description: Some("mock description".to_string()),
            }])
        }

        async fn import_skill(
            &mut self,
            _slug: &str,
        ) -> Result<vox_openclaw_runtime::ArsSkill, OpenClawAdapterError> {
            Err(OpenClawAdapterError::Other(
                "unused in this module".to_string(),
            ))
        }

        async fn list_subscriptions(&mut self) -> Result<serde_json::Value, OpenClawAdapterError> {
            Ok(serde_json::json!({
                "domains": ["ops.alerts"]
            }))
        }

        async fn subscribe_domain(
            &mut self,
            domain: &str,
        ) -> Result<serde_json::Value, OpenClawAdapterError> {
            Ok(serde_json::json!({
                "ok": true,
                "domain": domain
            }))
        }

        async fn unsubscribe_domain(
            &mut self,
            domain: &str,
        ) -> Result<serde_json::Value, OpenClawAdapterError> {
            Ok(serde_json::json!({
                "ok": true,
                "domain": domain
            }))
        }

        async fn notify_domain(
            &mut self,
            domain: &str,
            message: &str,
        ) -> Result<serde_json::Value, OpenClawAdapterError> {
            Ok(serde_json::json!({
                "ok": true,
                "domain": domain,
                "message": message
            }))
        }

        async fn gateway_call(
            &mut self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<serde_json::Value, OpenClawAdapterError> {
            Ok(serde_json::json!({
                "method": method,
                "params": params
            }))
        }
    }

    struct HookGuard;
    impl Drop for HookGuard {
        fn drop(&mut self) {
            test_hook::clear_connect_hook();
        }
    }

    fn install_mock_connect_hook() -> HookGuard {
        type ConnectFuture =
            Pin<Box<dyn Future<Output = Result<BoxedAgentAdapter, String>> + Send>>;
        type ConnectHook = dyn Fn() -> ConnectFuture + Send + Sync + 'static;
        let hook: Arc<ConnectHook> =
            Arc::new(|| Box::pin(async { Ok(Box::new(MockAdapter) as BoxedAgentAdapter) }));
        test_hook::set_connect_hook(hook);
        HookGuard
    }

    #[tokio::test]
    #[serial]
    async fn agent_gateway_call_returns_success_envelope() {
        let _guard = install_mock_connect_hook();
        let state = ServerState::new_test().await;
        let raw = agent_gateway_call(
            &state,
            OpenClawGatewayCallParams {
                method: "subscriptions.list".to_string(),
                params: serde_json::json!({ "domain": "ops.alerts" }),
            },
        )
        .await;
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        assert_eq!(
            parsed["data"]["method"],
            serde_json::Value::String("subscriptions.list".to_string())
        );
        assert_eq!(parsed["data"]["params"]["domain"], "ops.alerts");
    }

    #[tokio::test]
    #[serial]
    async fn agent_list_remote_returns_skill_list() {
        let _guard = install_mock_connect_hook();
        let state = ServerState::new_test().await;
        let raw = agent_list_remote(&state).await;
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        assert_eq!(parsed["data"]["skills"][0]["name"], "mock-skill");
    }
}
