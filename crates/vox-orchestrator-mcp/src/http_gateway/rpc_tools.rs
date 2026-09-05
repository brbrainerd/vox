use super::*;
pub(super) async fn http_tools(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }
    let role = match resolve_access_role(&state, &headers, Some(&connect.0)) {
        Ok(r) => r,
        Err(msg) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };
    let visible_tools = visible_tools_for_role(&state, role);
    let mut docs: Vec<ToolDescriptor> = Vec::new();
    for t in tool_registry() {
        let name = t.name.to_string();
        if !visible_tools.contains(&name) {
            continue;
        }
        let lane = t
            .meta
            .as_ref()
            .and_then(|m| m.0.get("vox_product_lane"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        docs.push(ToolDescriptor {
            name,
            description: t.description.map(|d| d.to_string()).unwrap_or_default(),
            input_schema: Value::Object((*t.input_schema).clone()),
            product_lane: lane,
        });
    }
    docs.sort_by(|a, b| a.name.cmp(&b.name));
    Json(serde_json::json!({ "success": true, "tools": docs })).into_response()
}
pub(super) async fn http_call_tool(
    State(state): State<GatewayState>,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(req): Json<ToolCallRequest>,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }
    let role = match resolve_access_role(&state, &headers, Some(&connect.0)) {
        Ok(r) => r,
        Err(msg) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response();
        }
    };
    call_tool_response(&state, req.name, req.args, Some(connect.0), role).await
}
pub(super) async fn call_tool_response(
    state: &GatewayState,
    name: String,
    args: Value,
    peer: Option<SocketAddr>,
    role: AccessRole,
) -> Response {
    Json(call_tool_response_value(state, name, args, peer, role).await).into_response()
}
pub(super) async fn call_tool_response_value(
    state: &GatewayState,
    name: String,
    args: Value,
    peer: Option<SocketAddr>,
    role: AccessRole,
) -> ToolCallResponse {
    let canonical = canonical_tool_name(name.as_str()).to_string();
    if !visible_tools_for_role(state, role).contains(&canonical) {
        return ToolCallResponse {
            success: false,
            is_error: true,
            result: serde_json::json!({
                "success": false,
                "error": format!("tool is not allowed for current gateway role: {canonical}")
            }),
        };
    }

    if let Some(p) = peer {
        tracing::info!(tool = %canonical, peer = %p, "mcp-http call_tool");
    } else {
        tracing::info!(tool = %canonical, "mcp-http call_tool");
    }

    let result = call_tool_json(&state.server_state, &canonical, args).await;
    let is_error = result
        .as_str()
        .map(tool_json_envelope_is_error)
        .unwrap_or(false);
    let rendered = match result {
        Value::String(s) => serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)),
        other => other,
    };
    ToolCallResponse {
        success: !is_error,
        is_error,
        result: rendered,
    }
}
pub(super) async fn call_tool_json(state: &ServerState, name: &str, args: Value) -> Value {
    match handle_tool_call(state, name, args).await {
        Ok(json) => Value::String(json),
        Err(e) => {
            let msg = format!("{e}");
            serde_json::to_value(ToolResult::<Value>::err_with_remediation(
                msg,
                "Verify tool args against /v1/tools schema and retry.",
            ))
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "success": false,
                    "error": "failed to serialize tool error envelope"
                })
            })
        }
    }
}
pub(super) async fn enforce_request_guards(
    state: &GatewayState,
    peer: &SocketAddr,
    headers: &HeaderMap,
) -> std::result::Result<(), Box<Response>> {
    let identity = request_identity(state, peer, headers);
    if let Err(msg) = enforce_auth(state, headers, Some(peer)) {
        return Err(Box::new(
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
        ));
    }
    if let Err(msg) = enforce_https_requirement(state, headers) {
        return Err(Box::new(
            (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
        ));
    }
    if let Err(msg) = enforce_rate_limit(state, &identity) {
        return Err(Box::new(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({ "error": msg })),
            )
                .into_response(),
        ));
    }
    Ok(())
}
