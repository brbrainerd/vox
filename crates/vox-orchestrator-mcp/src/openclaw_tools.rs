//! OpenClaw MCP tools backed by the native OpenClaw runtime adapter.

use crate::params::{
    OpenClawDomainParams, OpenClawGatewayCallParams, OpenClawImportParams, OpenClawNotifyParams,
    OpenClawSearchParams, ToolResult,
};
use crate::server_state::ServerState;
use vox_openclaw_runtime::{
    OpenClawClient, OpenClawConnectionOverrides, OpenClawDiscoveryOverrides, OpenClawRemoteConfig,
    OpenClawRuntimeAdapter, connect_runtime_adapter_with_overrides, resolve_openclaw_endpoints,
};

type BoxedOpenClawAdapter = Box<dyn OpenClawRuntimeAdapter + Send>;

#[cfg(not(test))]
type SharedOpenClawAdapter = std::sync::Arc<tokio::sync::Mutex<BoxedOpenClawAdapter>>;

#[cfg(not(test))]
static OPENCLAW_ADAPTER: tokio::sync::OnceCell<SharedOpenClawAdapter> =
    tokio::sync::OnceCell::const_new();

async fn connect_adapter_uncached() -> Result<BoxedOpenClawAdapter, String> {
    #[cfg(test)]
    if let Some(result) = test_hook::connect_if_configured().await {
        return result;
    }

    let secrets_token = resolve_secrets_token();
    connect_runtime_adapter_with_overrides(OpenClawConnectionOverrides {
        explicit_token: secrets_token,
        ..OpenClawConnectionOverrides::default()
    })
    .await
    .map(|adapter| Box::new(adapter) as BoxedOpenClawAdapter)
    .map_err(|e| format!("openclaw adapter connect failed: {e}"))
}

fn resolve_secrets_token() -> Option<String> {
    vox_secrets::resolve_secret(vox_secrets::SecretId::OpenClawToken)
        .expose()
        .map(std::string::ToString::to_string)
}

async fn connect_client() -> Result<OpenClawClient, String> {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides::default()).await;
    OpenClawClient::new(OpenClawRemoteConfig {
        gateway_url: resolved.http_gateway_url,
        auth_token: resolve_secrets_token(),
        verify_tls: true,
    })
    .map_err(|e| format!("openclaw client connect failed: {e}"))
}

/// Return resolved OpenClaw discovery configuration.
pub async fn openclaw_discover(_state: &ServerState) -> String {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides::default()).await;
    ToolResult::ok(serde_json::json!({
        "http_gateway_url": resolved.http_gateway_url,
        "ws_gateway_url": resolved.ws_gateway_url,
        "catalog_list_url": resolved.catalog_list_url,
        "catalog_search_url": resolved.catalog_search_url,
        "discovery_source": resolved.discovery_source,
        "cache_expires_at_ms": resolved.cache_expires_at_ms,
    }))
    .to_json()
}

/// Search remote OpenClaw skills by keyword.
pub async fn openclaw_search_remote(_state: &ServerState, params: OpenClawSearchParams) -> String {
    match connect_client().await {
        Ok(client) => match client.list_skills().await {
            Ok(skills) => {
                let q = params.query.to_lowercase();
                let matches: Vec<_> = skills
                    .into_iter()
                    .filter(|s| {
                        s.name.to_lowercase().contains(&q)
                            || s.description
                                .as_deref()
                                .unwrap_or_default()
                                .to_lowercase()
                                .contains(&q)
                    })
                    .collect();
                ToolResult::ok(serde_json::json!({
                    "query": params.query,
                    "count": matches.len(),
                    "skills": matches,
                }))
                .to_json()
            }
            Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
        },
        Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
    }
}

/// Import and optionally install an OpenClaw skill into local Vox skill registry.
pub async fn openclaw_import_skill(state: &ServerState, params: OpenClawImportParams) -> String {
    match connect_client().await {
        Ok(client) => {
            let imported = client.import_skill(&params.slug).await;
            match imported {
                Ok(skill) => {
                    if params.install {
                        match client
                            .import_and_install(&params.slug, &state.skill_registry)
                            .await
                        {
                            Ok(installed) => ToolResult::ok(serde_json::json!({
                                "status": "installed",
                                "skill_id": skill.id,
                                "name": skill.name,
                                "version": skill.version,
                                "install_result": installed,
                            }))
                            .to_json(),
                            Err(err) => ToolResult::ok(serde_json::json!({
                                "status": "imported_install_failed",
                                "skill_id": skill.id,
                                "name": skill.name,
                                "version": skill.version,
                                "error": err.to_string(),
                            }))
                            .to_json(),
                        }
                    } else {
                        ToolResult::ok(serde_json::json!({
                            "status": "imported",
                            "skill_id": skill.id,
                            "name": skill.name,
                            "version": skill.version,
                        }))
                        .to_json()
                    }
                }
                Err(err) => ToolResult::<serde_json::Value>::err(err.to_string()).to_json(),
            }
        }
        Err(err) => ToolResult::<serde_json::Value>::err(err).to_json(),
    }
}

/// Probe OpenClaw HTTP and WS connectivity.
pub async fn openclaw_health(_state: &ServerState) -> String {
    let resolved = resolve_openclaw_endpoints(OpenClawDiscoveryOverrides::default()).await;
    let http_probe_url = format!(
        "{}/v1/skills",
        resolved.http_gateway_url.trim_end_matches('/')
    );
    let http_client = vox_http_client::client_builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok();
    let http_status = match http_client {
        Some(client) => client
            .get(&http_probe_url)
            .send()
            .await
            .ok()
            .map(|r| r.status().as_u16()),
        None => None,
    };
    let ws_ok = connect_adapter_uncached().await.is_ok();
    ToolResult::ok(serde_json::json!({
        "ready": http_status.is_some() && ws_ok,
        "http_ok": http_status.is_some(),
        "http_status": http_status,
        "ws_ok": ws_ok,
        "http_gateway_url": resolved.http_gateway_url,
        "ws_gateway_url": resolved.ws_gateway_url,
        "discovery_source": resolved.discovery_source,
    }))
    .to_json()
}

/// List remote OpenClaw skills visible to this gateway identity.
pub async fn openclaw_list_remote(_state: &ServerState) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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

/// Generic OpenClaw gateway method call (WS control plane).
pub async fn openclaw_gateway_call(
    _state: &ServerState,
    params: OpenClawGatewayCallParams,
) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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

/// List OpenClaw gateway subscriptions for the current session.
pub async fn openclaw_subscriptions(_state: &ServerState) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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
pub async fn openclaw_subscribe(_state: &ServerState, params: OpenClawDomainParams) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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
pub async fn openclaw_unsubscribe(_state: &ServerState, params: OpenClawDomainParams) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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
pub async fn openclaw_notify(_state: &ServerState, params: OpenClawNotifyParams) -> String {
    #[cfg(test)]
    {
        match connect_adapter_uncached().await {
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
async fn shared_adapter() -> Result<SharedOpenClawAdapter, String> {
    OPENCLAW_ADAPTER
        .get_or_try_init(|| async {
            let adapter = connect_adapter_uncached().await?;
            Ok(std::sync::Arc::new(tokio::sync::Mutex::new(adapter)))
        })
        .await
        .map(std::sync::Arc::clone)
}

#[cfg(test)]
mod test_hook {
    use super::BoxedOpenClawAdapter;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex, OnceLock};

    type ConnectFuture = Pin<Box<dyn Future<Output = Result<BoxedOpenClawAdapter, String>> + Send>>;
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

    pub(super) async fn connect_if_configured() -> Option<Result<BoxedOpenClawAdapter, String>> {
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
    impl OpenClawRuntimeAdapter for MockAdapter {
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
            Pin<Box<dyn Future<Output = Result<BoxedOpenClawAdapter, String>> + Send>>;
        type ConnectHook = dyn Fn() -> ConnectFuture + Send + Sync + 'static;
        let hook: Arc<ConnectHook> =
            Arc::new(|| Box::pin(async { Ok(Box::new(MockAdapter) as BoxedOpenClawAdapter) }));
        test_hook::set_connect_hook(hook);
        HookGuard
    }

    #[tokio::test]
    #[serial]
    async fn openclaw_gateway_call_returns_success_envelope() {
        let _guard = install_mock_connect_hook();
        let state = ServerState::new_test().await;
        let raw = openclaw_gateway_call(
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
    async fn openclaw_list_remote_returns_skill_list() {
        let _guard = install_mock_connect_hook();
        let state = ServerState::new_test().await;
        let raw = openclaw_list_remote(&state).await;
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(true)));
        assert_eq!(parsed["data"]["skills"][0]["name"], "mock-skill");
    }

    #[tokio::test]
    #[serial]
    async fn openclaw_connect_failure_returns_error_envelope() {
        type ConnectFuture =
            Pin<Box<dyn Future<Output = Result<BoxedOpenClawAdapter, String>> + Send>>;
        type ConnectHook = dyn Fn() -> ConnectFuture + Send + Sync + 'static;
        let hook: Arc<ConnectHook> =
            Arc::new(|| Box::pin(async { Err("forced-test-connect-error".to_string()) }));
        test_hook::set_connect_hook(hook);
        let _guard = HookGuard;

        let state = ServerState::new_test().await;
        let raw = openclaw_subscriptions(&state).await;
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(parsed.get("success"), Some(&serde_json::json!(false)));
        let err = parsed["error"].as_str().unwrap_or_default();
        assert!(
            err.contains("forced-test-connect-error"),
            "unexpected error: {err}"
        );
    }
}
