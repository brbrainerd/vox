use super::*;
pub(super) async fn http_health(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    let identity = request_identity(&state, &connect.0, &headers);
    if let Err(msg) = enforce_rate_limit(&state, &identity) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response();
    }
    if state.health_auth_required
        && let Err(msg) = enforce_auth(&state, &headers, Some(&connect.0))
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response();
    }
    // Plain body for load balancers and strict `curl` gates (e.g. `eval.voxlang.org/health`).
    (StatusCode::OK, "ok").into_response()
}
pub(super) async fn http_info(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }
    let bind_host = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpHost)
        .expose()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_BIND_HOST)
        .to_string();
    let bind_port = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMcpHttpPort)
        .expose()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_BIND_PORT);
    let mut allowed_tools: Vec<String> = state.allowed_tools.iter().cloned().collect();
    allowed_tools.sort();
    let mut read_role_allowed_tools: Vec<String> = visible_tools_for_role(&state, AccessRole::Read)
        .into_iter()
        .collect();
    read_role_allowed_tools.sort();
    Json(GatewayInfo {
        enabled: true,
        bind_host,
        bind_port,
        auth_required: (state.bearer_token.is_some() || state.read_bearer_token.is_some())
            && !state.allow_unauthenticated,
        require_forwarded_https: state.require_forwarded_https,
        allowed_tools,
        read_role_allowed_tools,
        calls_per_minute: state.calls_per_minute,
    })
    .into_response()
}
pub(super) async fn http_mobile_workspace(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }
    let mut resp = Html(mobile_workspace_html()).into_response();
    resp.headers_mut().insert(
        "content-security-policy",
        "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'"
            .parse()
            .expect("valid CSP header value"),
    );
    resp
}
pub(super) async fn http_mobile_status(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }

    let git = call_tool_json(
        &state.server_state,
        "vox_git_status",
        Value::Object(serde_json::Map::new()),
    )
    .await;
    let orch = call_tool_json(
        &state.server_state,
        "vox_orchestrator_status",
        Value::Object(serde_json::Map::new()),
    )
    .await;
    Json(serde_json::json!({
        "success": true,
        "git_status": git,
        "orchestrator_status": orch
    }))
    .into_response()
}
