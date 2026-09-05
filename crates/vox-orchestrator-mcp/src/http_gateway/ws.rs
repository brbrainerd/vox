use super::*;
pub(super) async fn http_ws(
    State(state): State<GatewayState>,
    ws: WebSocketUpgrade,
    connect: ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Response {
    if let Err(resp) = enforce_request_guards(&state, &connect.0, &headers).await {
        return *resp;
    }

    let role_res = resolve_access_role(&state, &headers, Some(&connect.0));

    let role_opt = role_res.ok();
    let identity = request_identity(&state, &connect.0, &headers);
    ws.on_upgrade(move |socket| async move {
        handle_ws(socket, state, connect.0, identity, role_opt).await;
    })
}
pub(super) async fn handle_ws(
    mut socket: WebSocket,
    state: GatewayState,
    peer: SocketAddr,
    identity: String,
    mut role_opt: Option<AccessRole>,
) {
    let mut rx = state.server_state.orchestrator.event_bus().subscribe();
    let mut topic_rx = state.topic_tx.subscribe();
    let mut subscribed_topics: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                let Some(Ok(msg)) = msg else { break };
                match msg {
                    Message::Text(text) => {
                        let parsed: Result<WsMessageIn, _> = serde_json::from_str(&text);

                        if role_opt.is_none() {
                            let mut authenticated = false;
                            if let Ok(ref req) = parsed {
                                if req.msg_type == "auth" {
                                    if let Some(token_val) = req.args.as_ref().and_then(|v| v.get("token")).and_then(|t| t.as_str()) {
                                        if peer.ip().is_loopback() {
                                            if let Some(dt) = state.dashboard_token.as_ref() {
                                                if constant_time_eq(token_val.as_bytes(), dt.0.as_bytes()) {
                                                    role_opt = Some(AccessRole::Write);
                                                    authenticated = true;
                                                }
                                            }
                                        }
                                        if !authenticated {
                                            if let Some(bt) = state.bearer_token.as_ref() {
                                                if constant_time_eq(token_val.as_bytes(), bt.as_bytes()) {
                                                    role_opt = Some(AccessRole::Write);
                                                    authenticated = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if !authenticated {
                                let _ = socket.send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                    code: 4401,
                                    reason: "Unauthorized".into(),
                                }))).await;
                                break;
                            } else {
                                continue;
                            }
                        }

                        let role = role_opt.unwrap();
                        if let Err(msg) = enforce_rate_limit(&state, &identity) {
                            let _ = socket
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "error",
                                        "success": false,
                                        "is_error": true,
                                        "error": msg
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await;
                            break;
                        }
                        // Topic subscription control messages are handled inline
                        // (they mutate per-connection state rather than producing a tool result).
                        if let Ok(ref req) = parsed {
                            if req.msg_type == "subscribe" || req.msg_type == "unsubscribe" {
                                let topic = req
                                    .args
                                    .as_ref()
                                    .and_then(|v| v.get("topic"))
                                    .and_then(|t| t.as_str())
                                    .map(str::to_string);
                                let out = match topic {
                                    Some(topic) => {
                                        if req.msg_type == "subscribe" {
                                            subscribed_topics.insert(topic.clone());
                                        } else {
                                            subscribed_topics.remove(&topic);
                                        }
                                        WsMessageOut {
                                            id: req.id.clone(),
                                            msg_type: format!("{}_result", req.msg_type),
                                            success: true,
                                            is_error: false,
                                            data: serde_json::json!({
                                                "topic": topic,
                                                "subscribed": subscribed_topics.contains(&topic),
                                            }),
                                            error: None,
                                        }
                                    }
                                    None => WsMessageOut {
                                        id: req.id.clone(),
                                        msg_type: "error".to_string(),
                                        success: false,
                                        is_error: true,
                                        data: Value::Null,
                                        error: Some("missing 'topic' field".to_string()),
                                    },
                                };
                                if socket
                                    .send(Message::Text(
                                        serde_json::to_string(&out).unwrap_or_default().into(),
                                    ))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                continue;
                            }
                        }

                        let reply = match parsed {
                            Ok(req) => ws_handle_message(&state, req, peer, role).await,
                            Err(e) => WsMessageOut {
                                id: None,
                                msg_type: "error".to_string(),
                                success: false,
                                is_error: true,
                                data: Value::Null,
                                error: Some(format!("invalid websocket payload: {e}")),
                            },
                        };
                        if socket
                            .send(Message::Text(
                                serde_json::to_string(&reply)
                                    .unwrap_or_else(|_| {
                                        "{\"type\":\"error\",\"success\":false,\"is_error\":true}"
                                            .to_string()
                                    })
                                    .into(),
                            ))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Message::Ping(bytes) => {
                        if socket.send(Message::Pong(bytes)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            Ok(event) = rx.recv() => {
                let out = WsMessageOut {
                    id: None,
                    msg_type: "agent_event".to_string(),
                    success: true,
                    is_error: false,
                    data: serde_json::to_value(&event).unwrap_or(Value::Null),
                    error: None,
                };
                if socket.send(Message::Text(
                    serde_json::to_string(&out).unwrap_or_default().into()
                )).await.is_err() {
                    break;
                }
            }
            Ok(topic_msg) = topic_rx.recv() => {
                if !super::scientia_feed::should_forward(&subscribed_topics, &topic_msg.topic) {
                    continue;
                }
                let out = WsMessageOut {
                    id: None,
                    msg_type: topic_msg.topic.clone(),
                    success: true,
                    is_error: false,
                    data: topic_msg.data,
                    error: None,
                };
                if socket.send(Message::Text(
                    serde_json::to_string(&out).unwrap_or_default().into()
                )).await.is_err() {
                    break;
                }
            }
        }
    }
}
pub(super) async fn ws_handle_message(
    state: &GatewayState,
    req: WsMessageIn,
    peer: SocketAddr,
    role: AccessRole,
) -> WsMessageOut {
    match req.msg_type.as_str() {
        "list_tools" => {
            let mut tools: Vec<String> = visible_tools_for_role(state, role).into_iter().collect();
            tools.sort();
            WsMessageOut {
                id: req.id,
                msg_type: "list_tools_result".to_string(),
                success: true,
                is_error: false,
                data: serde_json::json!({ "tools": tools }),
                error: None,
            }
        }
        "call_tool" => {
            let Some(name) = req.name else {
                return WsMessageOut {
                    id: req.id,
                    msg_type: "call_tool_result".to_string(),
                    success: false,
                    is_error: true,
                    data: Value::Null,
                    error: Some("missing tool name".to_string()),
                };
            };
            let args = req
                .args
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            let result = call_tool_response_value(state, name, args, Some(peer), role).await;
            WsMessageOut {
                id: req.id,
                msg_type: "call_tool_result".to_string(),
                success: result.success,
                is_error: result.is_error,
                data: result.result,
                error: None,
            }
        }
        other => WsMessageOut {
            id: req.id,
            msg_type: "error".to_string(),
            success: false,
            is_error: true,
            data: Value::Null,
            error: Some(format!("unsupported message type: {other}")),
        },
    }
}
