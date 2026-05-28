use super::http_gateway::*;
use super::*;
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::{HeaderValue, header::AUTHORIZATION};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    async fn make_state(calls_per_minute: u32) -> GatewayState {
        let mut allowed = HashSet::new();
        allowed.insert("vox_orchestrator_status".to_string());
        GatewayState {
            server_state: ServerState::new_test().await,
            bearer_token: Some("secret-token".to_string()),
            read_bearer_token: Some("read-token".to_string()),
            allow_unauthenticated: false,
            health_auth_required: false,
            require_forwarded_https: false,
            trust_forwarded_for: false,
            allowed_tools: Arc::new(allowed),
            read_role_eligible_tools: Arc::new(
                ["vox_orchestrator_status".to_string()]
                    .into_iter()
                    .collect(),
            ),
            read_role_tools_override: None,
            calls_per_minute,
            rate_limiter: new_identity_rate_limiter(calls_per_minute),
        }
    }

    #[test]
    fn constant_time_eq_matches_expected() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[tokio::test]
    async fn auth_accepts_matching_bearer() {
        let state = make_state(5).await;
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            vox_http_client::bearer_auth_header("secret-token").unwrap(),
        );
        assert!(enforce_auth(&state, &headers).is_ok());
    }

    #[tokio::test]
    async fn auth_rejects_bad_bearer() {
        let state = make_state(5).await;
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, vox_http_client::bearer_auth_header("nope").unwrap());
        assert!(enforce_auth(&state, &headers).is_err());
    }

    #[tokio::test]
    async fn auth_accepts_read_bearer_as_read_role() {
        let state = make_state(5).await;
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, vox_http_client::bearer_auth_header("read-token").unwrap());
        assert_eq!(
            resolve_access_role(&state, &headers).expect("read token accepted"),
            AccessRole::Read
        );
    }

    #[test]
    fn parse_allowed_tools_default_has_safe_entries() {
        let set = parse_allowed_tools_from_value(None).expect("default allowlist should parse");
        assert!(set.contains("vox_orchestrator_status"));
        assert!(set.contains("vox_validate_file"));
    }

    #[test]
    fn parse_allowed_tools_rejects_unknown() {
        let err = parse_allowed_tools_from_value(Some("definitely_not_a_real_tool"))
            .expect_err("unknown tool should fail");
        assert!(err.to_string().contains("unknown tool"));
    }

    #[tokio::test]
    async fn rate_limit_blocks_after_budget() {
        let state = make_state(2).await;
        let key = "127.0.0.1".to_string();
        assert!(enforce_rate_limit(&state, &key).is_ok());
        assert!(enforce_rate_limit(&state, &key).is_ok());
        assert!(enforce_rate_limit(&state, &key).is_err());
    }

    #[tokio::test]
    async fn request_identity_prefers_forwarded_when_enabled() {
        let mut state = make_state(3).await;
        state.trust_forwarded_for = true;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.10, 10.0.0.1"),
        );
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        assert_eq!(
            request_identity(&state, &peer, &headers),
            "203.0.113.10".to_string()
        );
    }

    #[tokio::test]
    async fn ws_list_tools_filtered_to_allowlist() {
        let state = make_state(5).await;
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        let msg = WsMessageIn {
            id: Some("1".to_string()),
            msg_type: "list_tools".to_string(),
            name: None,
            args: None,
        };
        let out = ws_handle_message(&state, msg, peer, AccessRole::Write).await;
        assert!(out.success);
        let tools = out
            .data
            .get("tools")
            .and_then(|v| v.as_array())
            .expect("tools array");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].as_str(), Some("vox_orchestrator_status"));
    }

    #[tokio::test]
    async fn ws_call_tool_requires_name() {
        let state = make_state(5).await;
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        let msg = WsMessageIn {
            id: Some("x".to_string()),
            msg_type: "call_tool".to_string(),
            name: None,
            args: Some(Value::Null),
        };
        let out = ws_handle_message(&state, msg, peer, AccessRole::Write).await;
        assert!(!out.success);
        assert!(out.is_error);
        assert_eq!(out.error.as_deref(), Some("missing tool name"));
    }

    #[tokio::test]
    async fn read_role_is_limited_to_read_allowlist() {
        let mut state = make_state(5).await;
        state.allowed_tools = Arc::new(
            [
                "vox_orchestrator_status".to_string(),
                "vox_inline_edit".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        state.read_role_eligible_tools = Arc::new(
            ["vox_orchestrator_status".to_string()]
                .into_iter()
                .collect(),
        );
        let visible = visible_tools_for_role(&state, AccessRole::Read);
        assert!(visible.contains("vox_orchestrator_status"));
        assert!(!visible.contains("vox_inline_edit"));
    }

    #[test]
    fn metadata_read_role_eligible_contains_expected_tools() {
        let set = metadata_read_role_eligible_tools();
        assert!(set.contains("vox_orchestrator_status"));
        assert!(set.contains("vox_task_status"));
        assert!(set.contains("vox_git_status"));
    }

    #[tokio::test]
    async fn http_tools_read_role_hides_write_only_tools() {
        let mut state = make_state(5).await;
        state.allowed_tools = Arc::new(
            [
                "vox_orchestrator_status".to_string(),
                "vox_inline_edit".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        state.read_role_eligible_tools = Arc::new(
            ["vox_orchestrator_status".to_string()]
                .into_iter()
                .collect(),
        );
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, vox_http_client::bearer_auth_header("read-token").unwrap());
        let resp = http_tools(State(state), ConnectInfo(peer), headers).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let parsed: Value = serde_json::from_slice(&body).expect("json response");
        let tools = parsed
            .get("tools")
            .and_then(|v| v.as_array())
            .expect("tools array");
        let names: HashSet<String> = tools
            .iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .collect();
        assert!(names.contains("vox_orchestrator_status"));
        assert!(!names.contains("vox_inline_edit"));
    }

    #[tokio::test]
    async fn http_call_tool_read_role_denies_write_only_tool() {
        let mut state = make_state(5).await;
        state.allowed_tools = Arc::new(
            [
                "vox_orchestrator_status".to_string(),
                "vox_inline_edit".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        state.read_role_eligible_tools = Arc::new(
            ["vox_orchestrator_status".to_string()]
                .into_iter()
                .collect(),
        );
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, vox_http_client::bearer_auth_header("read-token").unwrap());
        let resp = http_call_tool(
            State(state),
            ConnectInfo(peer),
            headers,
            Json(ToolCallRequest {
                name: "vox_inline_edit".to_string(),
                args: Value::Object(serde_json::Map::new()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let parsed: ToolCallResponse = serde_json::from_slice(&body).expect("json response");
        assert!(!parsed.success);
        assert!(parsed.is_error);
        assert!(
            parsed
                .result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("not allowed for current gateway role")
        );
    }

    #[tokio::test]
    async fn http_info_exposes_effective_read_role_tools() {
        let mut state = make_state(5).await;
        state.allowed_tools = Arc::new(
            [
                "vox_orchestrator_status".to_string(),
                "vox_inline_edit".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        state.read_role_eligible_tools = Arc::new(
            ["vox_orchestrator_status".to_string()]
                .into_iter()
                .collect(),
        );
        let peer: SocketAddr = "127.0.0.1:1234".parse().expect("socket parse");
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, vox_http_client::bearer_auth_header("read-token").unwrap());
        let resp = http_info(State(state), ConnectInfo(peer), headers).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .expect("read body");
        let parsed: Value = serde_json::from_slice(&body).expect("json response");
        let read_tools = parsed
            .get("read_role_allowed_tools")
            .and_then(|v| v.as_array())
            .expect("read_role_allowed_tools array");
        let names: HashSet<String> = read_tools
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert!(names.contains("vox_orchestrator_status"));
        assert!(!names.contains("vox_inline_edit"));
    }

    #[tokio::test]
    async fn router_level_read_token_filters_tools_and_denies_write_only_call() {
        let mut state = make_state(10).await;
        state.allowed_tools = Arc::new(
            [
                "vox_orchestrator_status".to_string(),
                "vox_inline_edit".to_string(),
            ]
            .into_iter()
            .collect(),
        );
        state.read_role_eligible_tools = Arc::new(
            ["vox_orchestrator_status".to_string()]
                .into_iter()
                .collect(),
        );
        let app = Router::new()
            .route("/v1/tools", get(http_tools))
            .route("/v1/tools/call", post(http_call_tool))
            .with_state(state.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let _ = serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("test http client");
        let tools_resp = client
            .get(format!("http://{addr}/v1/tools"))
            .header("authorization", vox_http_client::bearer_auth_header_string("read-token"))
            .send()
            .await
            .expect("tools request");
        assert_eq!(tools_resp.status(), reqwest::StatusCode::OK);
        let tools_json: Value = tools_resp.json().await.expect("tools json");
        let names: HashSet<String> = tools_json["tools"]
            .as_array()
            .expect("tools array")
            .iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
            .collect();
        assert!(names.contains("vox_orchestrator_status"));
        assert!(!names.contains("vox_inline_edit"));

        let call_resp = client
            .post(format!("http://{addr}/v1/tools/call"))
            .header("authorization", vox_http_client::bearer_auth_header_string("read-token"))
            .json(&serde_json::json!({
                "name": "vox_inline_edit",
                "args": {}
            }))
            .send()
            .await
            .expect("call request");
        assert_eq!(call_resp.status(), reqwest::StatusCode::OK);
        let call_json: ToolCallResponse = call_resp.json().await.expect("call json");
        assert!(!call_json.success);
        assert!(call_json.is_error);
        assert!(
            call_json
                .result
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("not allowed for current gateway role")
        );
        server.abort();
    }

    #[test]
    #[allow(unsafe_code)]
    fn secrets_profile_lenient_vs_strict_for_gateway_token() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let token_key = "VOX_MCP_HTTP_BEARER_TOKEN";
        let prev_token = std::env::var(token_key).ok();
        unsafe {
            std::env::set_var("VOX_MCP_HTTP_BEARER_TOKEN", "gateway-env-token");
        }
        let resolver = vox_secrets::resolver::SecretResolver::new(vox_secrets::backend::NoopBackend);
        let lenient = resolver.resolve(
            vox_secrets::SecretId::VoxMcpHttpBearerToken,
            &vox_secrets::resolver::ResolveOptions {
                include_env: true,
                include_auth_json: false,
                include_populi_env: false,
                profile: vox_secrets::ResolveProfile::DevLenient,
            },
        );
        assert_eq!(lenient.expose(), Some("gateway-env-token"));

        let strict = resolver.resolve(
            vox_secrets::SecretId::VoxMcpHttpBearerToken,
            &vox_secrets::resolver::ResolveOptions {
                include_env: false,
                include_auth_json: false,
                include_populi_env: false,
                profile: vox_secrets::ResolveProfile::HardCutStrict,
            },
        );
        assert!(strict.expose().is_none());

        unsafe {
            match prev_token {
                Some(v) => std::env::set_var("VOX_MCP_HTTP_BEARER_TOKEN", v),
                None => std::env::remove_var("VOX_MCP_HTTP_BEARER_TOKEN"),
            }
        }
    }
}
