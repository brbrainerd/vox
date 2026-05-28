//! Optional network gateway for `vox-mcp` (HTTP + WebSocket).
//!
//! This gateway is disabled by default. When enabled, it exposes a bounded, authenticated
//! remote-control surface intended for mobile/browser clients that connect to a **remote** host
//! running the full Vox workspace and toolchain.

mod status;
use status::*;
mod rpc_tools;
use rpc_tools::*;
mod token;
mod ws;
use anyhow::{Context, Result};
use axum::Json;
use axum::extract::DefaultBodyLimit;
pub(super) use token::DashboardToken;
mod eval;
use eval::*;
mod origin_guard;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Router, serve};
use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use ws::*;

use crate::params::ToolResult;
use crate::server_state::{ServerState, tool_json_envelope_is_error};
use crate::{canonical_tool_name, handle_tool_call, tool_registry};
pub use vox_mcp_registry::TOOL_REGISTRY;

const DEFAULT_BIND_HOST: &str = "127.0.0.1";
const DEFAULT_BIND_PORT: u16 = 3921;
const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 120;

type IdentityRateLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

pub(super) fn new_identity_rate_limiter(calls_per_minute: u32) -> Arc<IdentityRateLimiter> {
    let n = NonZeroU32::new(calls_per_minute.max(1)).expect("rate limit min 1");
    Arc::new(RateLimiter::keyed(Quota::per_minute(n)))
}
const DEFAULT_ALLOWED_TOOLS: &[&str] = &[
    "vox_chat_message",
    "vox_plan",
    "vox_plan_status",
    "vox_inline_edit",
    "vox_validate_file",
    "vox_validate_source",
    "vox_git_status",
    "vox_git_diff",
    "vox_workspace_modules",
    "vox_orchestrator_status",
    "vox_task_status",
    "vox_repo_index_status",
    "vox_pause_agent",
    "vox_resume_agent",
    "vox_drain_agent",
    "vox_retire_agent",
    "vox_cancel_task",
    "vox_emergency_stop",
    "vox_rebalance",
    "vox_doubt_task",
    "vox_gamify_progress_snapshot",
    "vox_gamify_notification_ack",
    "vox_gamify_notifications_ack_all",
    "vox_budget_status",
    "vox_language_surface",
    "vox_pipeline_status",
    "vox_a2a_tasks",
    "vox_oplog",
    "vox_preference_set",
    "vox_preference_get",
    "vox_attention_reset",
    "vox_trust_override",
    "vox_set_agent_budget",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AccessRole {
    Read,
    Write,
}

#[derive(Clone)]
pub struct GatewayState {
    server_state: ServerState,
    bearer_token: Option<String>,
    read_bearer_token: Option<String>,
    dashboard_token: Option<DashboardToken>,
    allow_unauthenticated: bool,
    health_auth_required: bool,
    require_forwarded_https: bool,
    trust_forwarded_for: bool,
    allowed_tools: Arc<HashSet<String>>,
    read_role_eligible_tools: Arc<HashSet<String>>,
    read_role_tools_override: Option<Arc<HashSet<String>>>,
    calls_per_minute: u32,
    rate_limiter: Arc<IdentityRateLimiter>,
    pub public_eval_enabled: bool,
    pub public_eval_rate_limiter: Arc<IdentityRateLimiter>,
}

impl GatewayState {
    /// Minimal stub for tests — unauthenticated, no tools, no rate-limiting.
    #[cfg(test)]
    pub(crate) async fn for_test() -> Self {
        Self {
            server_state: ServerState::new_test().await,
            bearer_token: None,
            read_bearer_token: None,
            dashboard_token: None,
            allow_unauthenticated: true,
            health_auth_required: false,
            require_forwarded_https: false,
            trust_forwarded_for: false,
            allowed_tools: Arc::new(HashSet::new()),
            read_role_eligible_tools: Arc::new(HashSet::new()),
            read_role_tools_override: None,
            calls_per_minute: DEFAULT_RATE_LIMIT_PER_MINUTE,
            rate_limiter: new_identity_rate_limiter(DEFAULT_RATE_LIMIT_PER_MINUTE),
            public_eval_enabled: false,
            public_eval_rate_limiter: new_identity_rate_limiter(10),
        }
    }
}

#[derive(Debug, Serialize)]
struct GatewayInfo {
    enabled: bool,
    bind_host: String,
    bind_port: u16,
    auth_required: bool,
    require_forwarded_https: bool,
    allowed_tools: Vec<String>,
    read_role_allowed_tools: Vec<String>,
    calls_per_minute: u32,
}

#[derive(Debug, Serialize)]
struct ToolDescriptor {
    name: String,
    description: String,
    input_schema: Value,
    product_lane: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolCallRequest {
    name: String,
    #[serde(default)]
    args: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCallResponse {
    success: bool,
    is_error: bool,
    result: Value,
}

#[derive(Debug, Deserialize)]
struct WsMessageIn {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    msg_type: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    args: Option<Value>,
}

#[derive(Debug, Serialize)]
struct WsMessageOut {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "type")]
    msg_type: String,
    success: bool,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    error: Option<String>,
}

/// Returns true when `VOX_MCP_HTTP_ENABLED` is truthy.
pub fn http_gateway_enabled() -> bool {
    let result = read_bool_env(vox_secrets::SecretId::VoxMcpHttpEnabled).unwrap_or(false);
    let resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpEnabled);
    // Use tracing so this debug info honors RUST_LOG filters and never
    // corrupts stdout for callers piping the gateway output.
    tracing::debug!(
        enabled = result,
        status = ?resolved.status,
        source = ?resolved.source,
        "http gateway enablement resolved"
    );
    result
}

/// Build the production Axum [`Router`] for the HTTP gateway.
///
/// Extracted from [`spawn_http_gateway_if_enabled`] so tests can construct the same
/// router the production code uses without needing to bind a real TCP port.
pub fn build_app(state: GatewayState) -> Router {
    let app = Router::<GatewayState>::new()
        // /api/v2/* — versioned dashboard REST surface (envelope: { v, data } / { v, error })
        .merge(crate::services::routes::router())
        .route("/health", get(http_health))
        .route("/v1/info", get(http_info))
        .route("/v1/tools", get(http_tools))
        .route("/v1/tools/call", post(http_call_tool))
        .route("/v1/eval", post(http_eval))
        .route("/v1/ws", get(http_ws))
        .route("/v1/mobile", get(http_mobile_workspace))
        .route("/v1/mobile/status", get(http_mobile_status));



    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any)
        .expose_headers(tower_http::cors::Any);

    app.layer(cors)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            origin_guard::check_origin_allowlist,
        ))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(state)
}

/// Start the optional HTTP+WebSocket gateway in a background task.
pub fn spawn_http_gateway_if_enabled(
    state: ServerState,
) -> Result<Option<tokio::task::JoinHandle<()>>> {
    if !http_gateway_enabled() {
        return Ok(None);
    }

    let bind_host = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpHost)
        .expose()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BIND_HOST.to_string());
    let bind_port = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpPort)
        .expose()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_BIND_PORT);
    #[allow(unused_mut)]
    let mut bearer_token =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpBearerToken)
            .expose()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
    let read_bearer_token =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpReadBearerToken)
            .expose()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
    let allow_unauthenticated =
        read_bool_env(vox_secrets::SecretId::VoxMcpHttpAllowUnauthenticated).unwrap_or(false);

    let dashboard_token: Option<token::DashboardToken> = None;


    let public_eval_enabled =
        read_bool_env(vox_secrets::SecretId::VoxMcpHttpPublicEvalEnabled).unwrap_or(false);
    let public_eval_rate_limiter = new_identity_rate_limiter(10);

    if bearer_token.is_none()
        && read_bearer_token.is_none()
        && !allow_unauthenticated
        && !public_eval_enabled
    {
        anyhow::bail!(
            "VOX_MCP_HTTP_ENABLED=1 requires VOX_MCP_HTTP_BEARER_TOKEN or VOX_MCP_HTTP_READ_BEARER_TOKEN unless VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED=1 or VOX_MCP_HTTP_PUBLIC_EVAL_ENABLED=1 is explicitly set."
        );
    }
    let require_forwarded_https =
        read_bool_env(vox_secrets::SecretId::VoxMcpHttpRequireForwardedHttps).unwrap_or(false);
    let health_auth_required =
        read_bool_env(vox_secrets::SecretId::VoxMcpHttpHealthAuth).unwrap_or(false);
    let trust_forwarded_for =
        read_bool_env(vox_secrets::SecretId::VoxMcpHttpTrustXForwardedFor).unwrap_or(false);
    let calls_per_minute =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpRateLimitPerMinute)
            .expose()
            .and_then(|s| s.parse::<u32>().ok())
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_RATE_LIMIT_PER_MINUTE);
    let allowed_tools = Arc::new(parse_allowed_tools()?);
    let read_role_eligible_tools = Arc::new(metadata_read_role_eligible_tools());
    let read_role_tools_override = parse_read_role_allowed_tools_override()?.map(Arc::new);

    let gateway_state = GatewayState {
        server_state: state,
        bearer_token: bearer_token.clone(),
        read_bearer_token,
        dashboard_token: dashboard_token.clone(),
        allow_unauthenticated,
        health_auth_required,
        require_forwarded_https,
        trust_forwarded_for,
        allowed_tools: allowed_tools.clone(),
        read_role_eligible_tools,
        read_role_tools_override,
        calls_per_minute,
        rate_limiter: new_identity_rate_limiter(calls_per_minute),
        public_eval_enabled,
        public_eval_rate_limiter,
    };

    let app = build_app(gateway_state.clone());

    let addr: SocketAddr = format!("{bind_host}:{bind_port}")
        .parse()
        .with_context(|| format!("invalid VOX_MCP_HTTP_HOST/PORT: {bind_host}:{bind_port}"))?;
    let listener = std::net::TcpListener::bind(addr)
        .with_context(|| format!("failed to bind MCP HTTP gateway at {addr}"))?;
    listener
        .set_nonblocking(true)
        .context("failed to set nonblocking gateway listener")?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .context("failed to convert listener to tokio")?;

    let mut info_allowed_tools: Vec<String> = allowed_tools.iter().cloned().collect();
    info_allowed_tools.sort();
    let mut info_read_tools: Vec<String> = visible_tools_for_role(&gateway_state, AccessRole::Read)
        .into_iter()
        .collect();
    info_read_tools.sort();
    let info = GatewayInfo {
        enabled: true,
        bind_host: bind_host.clone(),
        bind_port,
        auth_required: (bearer_token.is_some() || gateway_state.read_bearer_token.is_some())
            && !allow_unauthenticated,
        require_forwarded_https,
        allowed_tools: info_allowed_tools,
        read_role_allowed_tools: info_read_tools,
        calls_per_minute,
    };
    tracing::info!(
        host = %info.bind_host,
        port = info.bind_port,
        auth_required = info.auth_required,
        tools = %info.allowed_tools.join(","),
        "VOX_MCP_HTTP_ENABLED: gateway started"
    );

    let handle = tokio::spawn(async move {
        if let Err(e) = serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        {
            tracing::error!("MCP HTTP gateway exited with error: {e}");
        }
    });
    Ok(Some(handle))
}

pub(super) fn enforce_auth(
    state: &GatewayState,
    headers: &HeaderMap,
    peer: Option<&SocketAddr>,
) -> std::result::Result<(), String> {
    resolve_access_role(state, headers, peer).map(|_| ())
}

pub(super) fn enforce_https_requirement(
    state: &GatewayState,
    headers: &HeaderMap,
) -> std::result::Result<(), String> {
    if !state.require_forwarded_https {
        return Ok(());
    }
    let forwarded_proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if forwarded_proto.eq_ignore_ascii_case("https") {
        Ok(())
    } else {
        Err("VOX_MCP_HTTP_REQUIRE_FORWARDED_HTTPS=1 requires X-Forwarded-Proto: https".to_string())
    }
}

pub(super) fn enforce_rate_limit(
    state: &GatewayState,
    identity: &String,
) -> std::result::Result<(), String> {
    state.rate_limiter.check_key(identity).map_err(|_| {
        format!(
            "rate limit exceeded (max {} requests/minute)",
            state.calls_per_minute
        )
    })
}

pub(super) fn parse_allowed_tools() -> Result<HashSet<String>> {
    parse_allowed_tools_from_value(
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpAllowedTools).expose(),
    )
}

pub(super) fn parse_read_role_allowed_tools_override() -> Result<Option<HashSet<String>>> {
    let explicit =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpReadRoleAllowedTools)
            .expose()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
    if let Some(csv) = explicit {
        parse_allowed_tools_from_value(Some(csv.as_str())).map(Some)
    } else {
        Ok(None)
    }
}

pub(super) fn metadata_read_role_eligible_tools() -> HashSet<String> {
    TOOL_REGISTRY
        .iter()
        .filter(|e| e.http_read_role_eligible)
        .map(|e| canonical_tool_name(e.name).to_string())
        .collect()
}

pub(super) fn resolve_access_role(
    state: &GatewayState,
    headers: &HeaderMap,
    peer: Option<&SocketAddr>,
) -> std::result::Result<AccessRole, String> {
    if state.allow_unauthenticated {
        tracing::warn!("Unauthenticated access allowed by VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED");
        return Ok(AccessRole::Write);
    }

    let mut got = String::new();

    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        got = auth
            .strip_prefix("Bearer ")
            .unwrap_or_default()
            .trim()
            .to_string();
    }

    if let Some(expected) = state.bearer_token.as_ref()
        && constant_time_eq(got.as_bytes(), expected.as_bytes())
    {
        return Ok(AccessRole::Write);
    }
    if let Some(expected_read) = state.read_bearer_token.as_ref()
        && constant_time_eq(got.as_bytes(), expected_read.as_bytes())
    {
        return Ok(AccessRole::Read);
    }

    if let Some(dt) = state.dashboard_token.as_ref()
        && constant_time_eq(got.as_bytes(), dt.0.as_bytes())
    {
        let is_loopback = peer.map(|p| p.ip().is_loopback()).unwrap_or(false);
        if is_loopback {
            return Ok(AccessRole::Write);
        }
    }

    Err("missing or invalid bearer token".to_string())
}

pub(super) fn visible_tools_for_role(state: &GatewayState, role: AccessRole) -> HashSet<String> {
    match role {
        AccessRole::Write => (*state.allowed_tools).clone(),
        AccessRole::Read => {
            let mut visible: HashSet<String> = state
                .allowed_tools
                .intersection(&state.read_role_eligible_tools)
                .cloned()
                .collect();
            if let Some(override_tools) = &state.read_role_tools_override {
                visible = visible.intersection(override_tools).cloned().collect();
            }
            visible
        }
    }
}

pub(super) fn parse_allowed_tools_from_value(raw: Option<&str>) -> Result<HashSet<String>> {
    let mut set = HashSet::new();
    if let Some(s) = raw {
        for v in s.split(',') {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                set.insert(canonical_tool_name(trimmed).to_string());
            }
        }
    }

    if set.is_empty() {
        for n in DEFAULT_ALLOWED_TOOLS.iter() {
            set.insert(canonical_tool_name(n).to_string());
        }
    }

    let mut registry_names: HashSet<&str> = HashSet::new();
    for e in TOOL_REGISTRY.iter() {
        registry_names.insert(e.name);
    }

    for name in set.iter() {
        let name_str: &str = name;
        if !registry_names.contains(name_str) {
            anyhow::bail!("unknown tool in VOX_MCP_HTTP_ALLOWED_TOOLS: {name}");
        }
    }
    Ok(set)
}

pub(super) fn request_identity(
    state: &GatewayState,
    peer: &SocketAddr,
    headers: &HeaderMap,
) -> String {
    if state.trust_forwarded_for
        && let Some(v) = headers.get("x-forwarded-for").and_then(|h| h.to_str().ok())
    {
        let first = v.split(',').next().map(str::trim).unwrap_or_default();
        if !first.is_empty() {
            return first.to_string();
        }
    }
    peer.ip().to_string()
}

pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let ai = *a.get(i).unwrap_or(&0);
        let bi = *b.get(i).unwrap_or(&0);
        diff |= ai ^ bi;
    }
    diff == 0
}

pub(super) fn read_bool_env(id: vox_secrets::SecretId) -> Option<bool> {
    vox_secrets::resolve_secret(id).expose().map(|v| {
        let t = v.trim();
        t == "1"
            || t.eq_ignore_ascii_case("true")
            || t.eq_ignore_ascii_case("yes")
            || t.eq_ignore_ascii_case("on")
    })
}

pub(super) fn mobile_workspace_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover" />
  <title>Vox Mobile Workspace</title>
  <style>
    :root { color-scheme: dark light; font-family: Inter, system-ui, sans-serif; }
    body { margin: 0; background: #0f1117; color: #e8eaf0; }
    main { max-width: 860px; margin: 0 auto; padding: 1rem; display: grid; gap: 0.75rem; }
    h1 { margin: 0; font-size: 1.25rem; }
    .card { background: #1a1d27; border: 1px solid #2e3244; border-radius: 10px; padding: 0.75rem; }
    label { display: block; font-size: 0.9rem; margin-bottom: 0.25rem; color: #9ca3b4; }
    input, select, textarea, button {
      width: 100%; box-sizing: border-box; font: inherit;
      border-radius: 8px; border: 1px solid #2e3244; background: #252836; color: #e8eaf0;
      padding: 0.6rem; min-height: 44px;
    }
    textarea { min-height: 140px; }
    button { cursor: pointer; background: #6366f1; border-color: #6366f1; }
    .row { display: grid; gap: 0.5rem; grid-template-columns: 1fr 1fr; }
    pre { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; font-size: 0.8rem; }
    .hint { color: #9ca3b4; font-size: 0.82rem; }
  </style>
</head>
<body>
  <main>
    <h1>Vox Mobile Workspace (bounded)</h1>
    <p class="hint">This UI is intentionally scoped for safe remote operations: status, planning/chat, and allowlisted tool calls.</p>
    <section class="card">
      <label for="token">Bearer token</label>
      <input id="token" placeholder="VOX_MCP_HTTP_BEARER_TOKEN" />
      <p class="hint">Used for Authorization: Bearer &lt;token&gt; on each call.</p>
    </section>
    <section class="card">
      <div class="row">
        <button id="refresh-status">Refresh status</button>
        <button id="list-tools">List allowlisted tools</button>
      </div>
      <div class="row" style="margin-top:0.5rem;">
        <button id="workspace-modules">Workspace modules</button>
        <button id="git-diff">Git diff</button>
      </div>
    </section>
    <section class="card">
      <label for="tool-name">Tool name</label>
      <input id="tool-name" value="vox_chat_message" />
      <label for="tool-args">Tool args (JSON object)</label>
      <textarea id="tool-args">{}</textarea>
      <button id="call-tool">Call tool</button>
    </section>
    <section class="card">
      <label>Output</label>
      <pre id="output">Ready.</pre>
    </section>
  </main>
  <script>
    const output = document.getElementById("output");
    const API_BASE = (() => {
      const path = window.location.pathname;
      return path.endsWith("/mobile") ? path.slice(0, -"/mobile".length) : "/v1";
    })();
    const tokenInput = document.getElementById("token");
    tokenInput.value = localStorage.getItem("voxMcpHttpToken") || "";
    tokenInput.addEventListener("change", () => {
      localStorage.setItem("voxMcpHttpToken", tokenInput.value.trim());
    });
    function authHeaders() {
      const token = tokenInput.value.trim();
      const headers = { "Content-Type": "application/json" };
      if (token) headers["Authorization"] = `Bearer ${token}`;
      return headers;
    }
    async function showJson(resp) {
      const txt = await resp.text();
      try { output.textContent = JSON.stringify(JSON.parse(txt), null, 2); }
      catch { output.textContent = txt; }
    }
    document.getElementById("refresh-status").addEventListener("click", async () => {
      const resp = await fetch(`${API_BASE}/mobile/status`, { headers: authHeaders() });
      await showJson(resp);
    });
    document.getElementById("list-tools").addEventListener("click", async () => {
      const resp = await fetch(`${API_BASE}/tools`, { headers: authHeaders() });
      await showJson(resp);
    });
    document.getElementById("workspace-modules").addEventListener("click", async () => {
      const resp = await fetch(`${API_BASE}/tools/call`, {
        method: "POST",
        headers: authHeaders(),
        body: JSON.stringify({ name: "vox_workspace_modules", args: {} }),
      });
      await showJson(resp);
    });
    document.getElementById("git-diff").addEventListener("click", async () => {
      const resp = await fetch(`${API_BASE}/tools/call`, {
        method: "POST",
        headers: authHeaders(),
        body: JSON.stringify({ name: "vox_git_diff", args: {} }),
      });
      await showJson(resp);
    });
    document.getElementById("call-tool").addEventListener("click", async () => {
      const name = document.getElementById("tool-name").value.trim();
      let args = {};
      try { args = JSON.parse(document.getElementById("tool-args").value || "{}"); }
      catch (err) {
        output.textContent = `Invalid JSON args: ${err}`;
        return;
      }
      const resp = await fetch(`${API_BASE}/tools/call`, {
        method: "POST",
        headers: authHeaders(),
        body: JSON.stringify({ name, args }),
      });
      await showJson(resp);
    });
  </script>
</body>
</html>
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_length_256_multiple_not_equal() {
        // Lengths differ by 256 — before the fix, `(256 ^ 0) as u8 == 0` produces a false positive.
        let a = vec![0u8; 256];
        let b: &[u8] = &[];
        assert!(
            !constant_time_eq(&a, b),
            "empty slice must not equal 256-byte slice"
        );
        let c = vec![0u8; 512];
        assert!(
            !constant_time_eq(&a, &c),
            "256-byte slice must not equal 512-byte slice"
        );
    }

    /// Exercises `build_app` end-to-end: proves the production router factory
    /// wires `crate::services::routes::router()` correctly by hitting the
    /// `/api/v2/health` endpoint (which has no `ConnectInfo` dependency).
    #[tokio::test]
    async fn build_app_wires_api_v2_routes() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;
        let state = GatewayState::for_test().await;
        let app = build_app(state);
        let req = Request::builder()
            .uri("/api/v2/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
