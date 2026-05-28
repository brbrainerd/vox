#![allow(missing_docs)]
#![allow(clippy::await_holding_lock)]
// Env lock serializes transport tests across whole async bodies.
// `#[serial]` serializes all tests in this binary: `VOX_MESH_*` env is process-wide (see e.g. `VOX_MESH_A2A_MAX_MESSAGES`).
//! Axum readiness polls `/health`; prune/maintenance paths use bounded condition waits instead of fixed sleeps.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Mutex;
use std::time::Duration;

use serial_test::serial;
use vox_populi::http_client::PopuliHttpClient;
use vox_populi::transport::{
    AdminMaintenanceRequest, AdminQuarantineRequest, PopuliHttpAuth, PopuliTransportState,
};
use vox_populi::{node_record_for_current_process, transport};

static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Bounded async polling helper for integration tests; panics with `label` on timeout.
async fn wait_until_async<F, Fut>(
    label: &str,
    timeout: Duration,
    interval: Duration,
    mut condition: F,
) where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if condition().await {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("{label}: timed out after {timeout:?}");
        }
        tokio::time::sleep(interval).await;
    }
}

/// Waits until the spawned Axum server responds on `/health` instead of a fixed sleep.
async fn wait_populi_http_ready(base: &str) {
    let url = format!("{base}/health");
    wait_until_async(
        "Populi HTTP server accepting connections (/health)",
        Duration::from_secs(15),
        Duration::from_millis(5),
        || async {
            match reqwest::get(&url).await {
                Ok(r) => r.status().is_success(),
                Err(_) => false,
            }
        },
    )
    .await;
}

#[tokio::test]
#[serial]
async fn join_list_heartbeat_roundtrip() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let client = PopuliHttpClient::new(&base);
    let empty = client.list_nodes().await.unwrap();
    assert!(empty.nodes.is_empty());

    let node = node_record_for_current_process("node-a".into(), Some(bound.to_string()));
    let joined = client.join(&node).await.unwrap();
    assert_eq!(joined.id, "node-a");

    let listed = client.list_nodes().await.unwrap();
    assert_eq!(listed.nodes.len(), 1);

    let beat = client.heartbeat(&node).await.unwrap();
    assert!(beat.last_seen_unix_ms >= node.last_seen_unix_ms);

    server.abort();
}

#[tokio::test]
#[serial]
async fn leave_removes_node() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let client = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("gone".into(), None);
    client.join(&node).await.unwrap();
    assert_eq!(client.list_nodes().await.unwrap().nodes.len(), 1);

    assert!(client.leave("gone").await.unwrap());
    assert!(client.list_nodes().await.unwrap().nodes.is_empty());
    assert!(!client.leave("gone").await.unwrap());

    server.abort();
}

#[tokio::test]
#[serial]
async fn mesh_token_requires_bearer() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(
        state,
        PopuliHttpAuth::Bearer("unit-test-populi-token".into()),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let no_auth = PopuliHttpClient::new(&base);
    assert!(no_auth.list_nodes().await.is_err());

    let authed = PopuliHttpClient::new(&base).with_bearer("unit-test-populi-token");
    assert!(authed.list_nodes().await.is_ok());

    server.abort();
}

#[tokio::test]
#[serial]
async fn health_ok_without_bearer_when_token_required() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(
        state,
        transport::PopuliHttpAuth::Bearer("unit-test-populi-token".into()),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let url = format!("{base}/health");
    let r = reqwest::get(&url).await.unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    assert_eq!(r.text().await.unwrap(), "ok\n");

    server.abort();
}

#[tokio::test]
#[serial]
async fn join_rejected_when_scope_mismatch() {
    let state = PopuliTransportState::with_required_scope(Some("cluster-a".into()));
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let client = PopuliHttpClient::new(&base);
    let mut node = node_record_for_current_process("wrong-scope".into(), None);
    node.scope_id = Some("cluster-b".into());
    let err = client.join(&node).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("403") || msg.contains("Forbidden"),
        "unexpected error: {msg}"
    );

    server.abort();
}

#[tokio::test]
#[serial]
async fn join_ok_when_scope_matches() {
    let state = PopuliTransportState::with_required_scope(Some("cluster-a".into()));
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;
    let client = PopuliHttpClient::new(&base);
    let mut node = node_record_for_current_process("scoped-node".into(), None);
    node.scope_id = Some("cluster-a".into());
    client.join(&node).await.unwrap();
    assert_eq!(client.list_nodes().await.unwrap().nodes.len(), 1);

    server.abort();
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn list_nodes_omits_stale_entries_when_server_prune_env_set() {
    let _guard = ENV_MUTEX.lock().expect("env lock");
    let prev = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshServerStalePruneMs)
        .expose()
        .map(|s| s.to_string());
    unsafe {
        std::env::set_var("VOX_MESH_SERVER_STALE_PRUNE_MS", "5");
    }

    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();

    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let client = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("stale-a".into(), None);
    client.join(&node).await.unwrap();
    assert_eq!(client.list_nodes().await.unwrap().nodes.len(), 1);

    wait_until_async(
        "stale mesh nodes pruned from list (VOX_MESH_SERVER_STALE_PRUNE_MS)",
        Duration::from_secs(5),
        Duration::from_millis(5),
        || async { client.list_nodes().await.unwrap().nodes.is_empty() },
    )
    .await;

    server.abort();
    unsafe {
        match prev {
            Some(v) => std::env::set_var("VOX_MESH_SERVER_STALE_PRUNE_MS", v),
            None => std::env::remove_var("VOX_MESH_SERVER_STALE_PRUNE_MS"),
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn a2a_deliver_respects_in_memory_cap() {
    let _guard = ENV_MUTEX.lock().expect("env lock");
    let prev = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshA2aMaxMessages)
        .expose()
        .map(|s| s.to_string());
    unsafe {
        std::env::set_var("VOX_MESH_A2A_MAX_MESSAGES", "3");
    }

    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = reqwest::Client::new();
    for i in 0..5 {
        let r = http
            .post(format!("{base}/v1/populi/a2a/deliver"))
            .json(&serde_json::json!({
                "sender_agent_id": "1",
                "receiver_agent_id": "2",
                "message_type": "t",
                "payload": format!("{i}")
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), reqwest::StatusCode::OK);
    }
    let inbox = http
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({ "receiver_agent_id": "2" }))
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = inbox.json().await.unwrap();
    let n = body["messages"].as_array().map_or(0, |a| a.len());
    assert!(n <= 3, "expected cap 3, got {n}: {body:?}");

    server.abort();
    unsafe {
        match prev {
            Some(v) => std::env::set_var("VOX_MESH_A2A_MAX_MESSAGES", v),
            None => std::env::remove_var("VOX_MESH_A2A_MAX_MESSAGES"),
        }
    }
}

#[tokio::test]
#[serial]
async fn a2a_deliver_rejects_non_numeric_agent_ids() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = reqwest::Client::new();
    for (sender, receiver) in [("bad", "1"), ("1", "r"), ("", "1"), ("1", "  ")] {
        let r = http
            .post(format!("{base}/v1/populi/a2a/deliver"))
            .json(&serde_json::json!({
                "sender_agent_id": sender,
                "receiver_agent_id": receiver,
                "message_type": "t",
                "payload": "x"
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            r.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "sender={sender:?} receiver={receiver:?}"
        );
    }

    server.abort();
}

#[tokio::test]
#[serial]
async fn oversized_json_body_returns_413() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let pad = "x".repeat(600 * 1024);
    let http = reqwest::Client::new();
    let r = http
        .post(format!("{base}/v1/populi/join"))
        .json(&serde_json::json!({
            "id": pad,
            "capabilities": {},
            "version": "test",
            "last_seen_unix_ms": 0
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        reqwest::StatusCode::PAYLOAD_TOO_LARGE,
        "expected 413 for oversized join body"
    );

    server.abort();
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn bootstrap_exchange_works_once() {
    let _guard = ENV_MUTEX.lock().expect("env lock");
    let prev_bootstrap = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshBootstrapToken)
        .expose()
        .map(|s| s.to_string());
    let prev_expires =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshBootstrapExpiresUnixMs)
            .expose()
            .map(|s| s.to_string());
    let prev_token = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshToken)
        .expose()
        .map(|s| s.to_string());
    let prev_scope = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshScopeId)
        .expose()
        .map(|s| s.to_string());

    let bootstrap = "bootstrap-unit-test-token";
    // SAFETY: serialized by `ENV_MUTEX`, restored at test end.
    unsafe {
        std::env::set_var("VOX_MESH_BOOTSTRAP_TOKEN", bootstrap);
        std::env::set_var(
            "VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS",
            (vox_populi::wall_clock_unix_ms() + 120_000).to_string(),
        );
        std::env::set_var(
            vox_secrets::SecretId::VoxMeshToken.spec().canonical_env,
            "mesh-unit-token",
        );
        std::env::set_var("VOX_MESH_SCOPE_ID", "scope-unit");
    }

    let state = PopuliTransportState::new_for_serve();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let client = reqwest::Client::new();
    let first = client
        .post(format!("{base}/v1/populi/bootstrap/exchange"))
        .json(&serde_json::json!({ "bootstrap_token": bootstrap }))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), reqwest::StatusCode::OK);
    let payload: serde_json::Value = first.json().await.unwrap();
    assert_eq!(payload["mesh_token"], "mesh-unit-token");
    assert_eq!(payload["scope_id"], "scope-unit");

    let second = client
        .post(format!("{base}/v1/populi/bootstrap/exchange"))
        .json(&serde_json::json!({ "bootstrap_token": bootstrap }))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::GONE);

    server.abort();
    unsafe {
        match prev_bootstrap {
            Some(v) => std::env::set_var("VOX_MESH_BOOTSTRAP_TOKEN", v),
            None => std::env::remove_var("VOX_MESH_BOOTSTRAP_TOKEN"),
        }
        match prev_expires {
            Some(v) => std::env::set_var("VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS", v),
            None => std::env::remove_var("VOX_MESH_BOOTSTRAP_EXPIRES_UNIX_MS"),
        }
        match prev_token {
            Some(v) => {
                std::env::set_var(vox_secrets::SecretId::VoxMeshToken.spec().canonical_env, v)
            }
            None => std::env::remove_var(vox_secrets::SecretId::VoxMeshToken.spec().canonical_env),
        }
        match prev_scope {
            Some(v) => std::env::set_var("VOX_MESH_SCOPE_ID", v),
            None => std::env::remove_var("VOX_MESH_SCOPE_ID"),
        }
    }
}

#[tokio::test]
#[serial]
async fn quarantine_blocks_claim_until_cleared() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("q-worker".into(), None);
    http.join(&node).await.unwrap();
    http.admin_quarantine(&AdminQuarantineRequest {
        node_id: "q-worker".into(),
        quarantined: true,
    })
    .await
    .unwrap();
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "0".into(),
        receiver_agent_id: "99".into(),
        message_type: vox_populi::transport::A2A_MESSAGE_JOB_SUBMIT.into(),
        payload: "{}".into(),
        idempotency_key: None,
        privacy_class: Some("private".into()),
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    })
    .await
    .unwrap();
    let r = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "99",
            "claimer_node_id": "q-worker",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["messages"].as_array().unwrap().len(), 0);
    http.admin_quarantine(&AdminQuarantineRequest {
        node_id: "q-worker".into(),
        quarantined: false,
    })
    .await
    .unwrap();
    let r2i = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "99",
            "claimer_node_id": "q-worker",
        }))
        .send()
        .await
        .unwrap();
    let body2: serde_json::Value = r2i.json().await.unwrap();
    assert_eq!(body2["messages"].as_array().unwrap().len(), 1);
    server.abort();
}

#[tokio::test]
#[serial]
async fn maintenance_blocks_claim_and_lease_renew() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    let worker = node_record_for_current_process("maint-worker".into(), None);
    http.join(&worker).await.unwrap();
    http.admin_maintenance(&AdminMaintenanceRequest {
        node_id: "maint-worker".into(),
        maintenance: true,
        maintenance_until_unix_ms: None,
        maintenance_for_ms: None,
    })
    .await
    .unwrap();

    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "0".into(),
        receiver_agent_id: "17".into(),
        message_type: "x".into(),
        payload: "{}".into(),
        idempotency_key: None,
        privacy_class: Some("private".into()),
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    })
    .await
    .unwrap();

    let inbox_blocked: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "17",
            "claimer_node_id": "maint-worker",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(inbox_blocked["messages"].as_array().unwrap().len(), 0);

    http.admin_maintenance(&AdminMaintenanceRequest {
        node_id: "maint-worker".into(),
        maintenance: false,
        maintenance_until_unix_ms: None,
        maintenance_for_ms: None,
    })
    .await
    .unwrap();

    let inbox_claimed: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "17",
            "claimer_node_id": "maint-worker",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mid = inbox_claimed["messages"][0]["id"].as_u64().unwrap();

    http.admin_maintenance(&AdminMaintenanceRequest {
        node_id: "maint-worker".into(),
        maintenance: true,
        maintenance_until_unix_ms: None,
        maintenance_for_ms: None,
    })
    .await
    .unwrap();
    let renew_resp = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/lease-renew"))
        .json(&serde_json::json!({
            "receiver_agent_id": "17",
            "message_id": mid,
            "claimer_node_id": "maint-worker",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(renew_resp.status(), reqwest::StatusCode::FORBIDDEN);

    server.abort();
}

#[tokio::test]
#[serial]
async fn maintenance_deadline_expires_and_claims_resume() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    let worker = node_record_for_current_process("maint-deadline-worker".into(), None);
    http.join(&worker).await.unwrap();
    http.admin_maintenance(&AdminMaintenanceRequest {
        node_id: "maint-deadline-worker".into(),
        maintenance: true,
        maintenance_until_unix_ms: None,
        maintenance_for_ms: Some(80),
    })
    .await
    .unwrap();

    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "0".into(),
        receiver_agent_id: "901".into(),
        message_type: "x".into(),
        payload: "{}".into(),
        idempotency_key: None,
        privacy_class: Some("private".into()),
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    })
    .await
    .unwrap();

    let blocked: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "901",
            "claimer_node_id": "maint-deadline-worker",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(blocked["messages"].as_array().unwrap().len(), 0);

    let inbox_client = reqwest::Client::new();
    wait_until_async(
        "maintenance_for_ms elapsed; inbox claims resume",
        Duration::from_secs(5),
        Duration::from_millis(10),
        || async {
            let claimed: serde_json::Value = inbox_client
                .post(format!("{base}/v1/populi/a2a/inbox"))
                .json(&serde_json::json!({
                    "receiver_agent_id": "901",
                    "claimer_node_id": "maint-deadline-worker",
                }))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            claimed["messages"].as_array().unwrap().len() == 1
        },
    )
    .await;

    server.abort();
}

#[tokio::test]
#[serial]
async fn a2a_lease_renew_requires_holder() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("lease-a".into(), None);
    http.join(&node).await.unwrap();
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "0".into(),
        receiver_agent_id: "7".into(),
        message_type: "x".into(),
        payload: "{}".into(),
        idempotency_key: None,
        privacy_class: Some("private".into()),
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    })
    .await
    .unwrap();
    let inbox: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "7",
            "claimer_node_id": "lease-a",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mid = inbox["messages"][0]["id"].as_u64().unwrap();
    http.relay_a2a_lease_renew(&vox_populi::transport::A2ALeaseRenewRequest {
        receiver_agent_id: "7".into(),
        message_id: mid,
        claimer_node_id: "lease-a".into(),
    })
    .await
    .unwrap();
    let node_b = node_record_for_current_process("lease-b".into(), None);
    http.join(&node_b).await.unwrap();
    let bad = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/lease-renew"))
        .json(&serde_json::json!({
            "receiver_agent_id": "7",
            "message_id": mid,
            "claimer_node_id": "lease-b",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), reqwest::StatusCode::CONFLICT);
    server.abort();
}

#[tokio::test]
#[serial]
async fn remote_exec_lease_grant_renew_release_happy_path() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("exec-worker".into(), None);
    http.join(&node).await.unwrap();

    let g1 = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-worker".into(),
            scope_key: "task:unit-a".into(),
        })
        .await
        .unwrap();
    assert_eq!(g1.holder_node_id, "exec-worker");
    assert_eq!(g1.scope_key, "task:unit-a");
    assert!(!g1.lease_id.is_empty());
    assert!(g1.expires_unix_ms > 0);

    http.exec_lease_renew(&vox_populi::transport::RemoteExecLeaseRenewRequest {
        lease_id: g1.lease_id.clone(),
        claimer_node_id: "exec-worker".into(),
    })
    .await
    .unwrap();

    http.exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
        lease_id: g1.lease_id.clone(),
        claimer_node_id: "exec-worker".into(),
    })
    .await
    .unwrap();

    let renew_gone = http
        .exec_lease_renew(&vox_populi::transport::RemoteExecLeaseRenewRequest {
            lease_id: g1.lease_id.clone(),
            claimer_node_id: "exec-worker".into(),
        })
        .await;
    let err = renew_gone.expect_err("renew after release should fail");
    assert!(
        err.is_http_status(404),
        "expected structured 404 status, got {err}"
    );

    server.abort();
}

#[tokio::test]
#[serial]
async fn exec_lease_list_reflects_grant_and_sweep() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    assert!(http.list_exec_leases().await.unwrap().leases.is_empty());

    let node = node_record_for_current_process("exec-list-worker".into(), None);
    http.join(&node).await.unwrap();
    let g1 = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-list-worker".into(),
            scope_key: "task:list-a".into(),
        })
        .await
        .unwrap();
    let listed = http.list_exec_leases().await.unwrap();
    assert_eq!(listed.leases.len(), 1);
    assert_eq!(listed.leases[0].lease_id, g1.lease_id);
    assert_eq!(listed.leases[0].holder_node_id, "exec-list-worker");

    http.exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
        lease_id: g1.lease_id,
        claimer_node_id: "exec-list-worker".into(),
    })
    .await
    .unwrap();
    assert!(http.list_exec_leases().await.unwrap().leases.is_empty());

    server.abort();
}

#[tokio::test]
#[serial]
async fn admin_exec_lease_revoke_removes_row_without_holder_release() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("exec-revoke-worker".into(), None);
    http.join(&node).await.unwrap();
    let g = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-revoke-worker".into(),
            scope_key: "task:admin-revoke".into(),
        })
        .await
        .unwrap();
    assert_eq!(http.list_exec_leases().await.unwrap().leases.len(), 1);

    http.admin_exec_lease_revoke(&vox_populi::transport::AdminExecLeaseRevokeRequest {
        lease_id: g.lease_id.clone(),
    })
    .await
    .unwrap();
    assert!(http.list_exec_leases().await.unwrap().leases.is_empty());

    let missing = http
        .admin_exec_lease_revoke(&vox_populi::transport::AdminExecLeaseRevokeRequest {
            lease_id: g.lease_id,
        })
        .await;
    assert!(missing.is_err(), "second revoke should 404",);
    let err = missing.unwrap_err();
    assert!(err.is_http_status(404), "expected 404, got {err}");

    server.abort();
}

#[tokio::test]
#[serial]
async fn exec_lease_release_succeeds_under_maintenance_for_holder_cleanup() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("exec-maint-release".into(), None);
    http.join(&node).await.unwrap();

    let g1 = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-maint-release".into(),
            scope_key: "task:drain-test".into(),
        })
        .await
        .unwrap();

    http.admin_maintenance(&AdminMaintenanceRequest {
        node_id: "exec-maint-release".into(),
        maintenance: true,
        maintenance_until_unix_ms: None,
        maintenance_for_ms: None,
    })
    .await
    .unwrap();

    let renew_blocked = http
        .exec_lease_renew(&vox_populi::transport::RemoteExecLeaseRenewRequest {
            lease_id: g1.lease_id.clone(),
            claimer_node_id: "exec-maint-release".into(),
        })
        .await;
    assert!(
        renew_blocked.is_err(),
        "renew should fail while claimer is in maintenance"
    );

    http.exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
        lease_id: g1.lease_id.clone(),
        claimer_node_id: "exec-maint-release".into(),
    })
    .await
    .expect("release must succeed so scope_key can be cleared during drain");

    server.abort();
}

#[tokio::test]
#[serial]
async fn remote_exec_lease_grant_idempotent_for_same_holder() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let node = node_record_for_current_process("exec-same".into(), None);
    http.join(&node).await.unwrap();

    let g1 = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-same".into(),
            scope_key: "scope-idem".into(),
        })
        .await
        .unwrap();
    let g2 = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-same".into(),
            scope_key: "scope-idem".into(),
        })
        .await
        .unwrap();
    assert_eq!(g1.lease_id, g2.lease_id);
    assert!(g2.expires_unix_ms >= g1.expires_unix_ms);

    server.abort();
}

#[tokio::test]
#[serial]
async fn remote_exec_lease_renew_requires_holder() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    http.join(&node_record_for_current_process("rex-a".into(), None))
        .await
        .unwrap();
    http.join(&node_record_for_current_process("rex-b".into(), None))
        .await
        .unwrap();
    let g = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "rex-a".into(),
            scope_key: "s-hold".into(),
        })
        .await
        .unwrap();
    let bad = reqwest::Client::new()
        .post(format!("{base}/v1/populi/exec/lease/renew"))
        .json(&serde_json::json!({
            "lease_id": g.lease_id,
            "claimer_node_id": "rex-b",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), reqwest::StatusCode::CONFLICT);
    server.abort();
}

#[tokio::test]
#[serial]
async fn remote_exec_lease_second_holder_gets_409() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    http.join(&node_record_for_current_process("exec-a".into(), None))
        .await
        .unwrap();
    http.join(&node_record_for_current_process("exec-b".into(), None))
        .await
        .unwrap();
    http.exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
        claimer_node_id: "exec-a".into(),
        scope_key: "shared-scope".into(),
    })
    .await
    .unwrap();
    let err = http
        .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
            claimer_node_id: "exec-b".into(),
            scope_key: "shared-scope".into(),
        })
        .await
        .unwrap_err();
    assert!(err.is_http_status(409), "expected 409 conflict, got {err}");
    server.abort();
}

#[tokio::test]
#[serial]
async fn a2a_inbox_non_claimer_honors_max_messages() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    for i in 0..3 {
        http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "9".into(),
            message_type: format!("m{i}"),
            payload: "{}".into(),
            idempotency_key: None,
            privacy_class: Some("private".into()),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            priority: 128,
            task_kind: None,
            model_id: None,
            traceparent: None,
        })
        .await
        .unwrap();
    }

    let limited: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/v1/populi/a2a/inbox"))
        .json(&serde_json::json!({
            "receiver_agent_id": "9",
            "max_messages": 1
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        limited["messages"].as_array().map(|a| a.len()).unwrap_or(0),
        1
    );

    server.abort();
}

#[tokio::test]
#[serial]
async fn a2a_inbox_non_claimer_honors_before_message_cursor() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    for i in 0..3 {
        http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "10".into(),
            message_type: format!("m{i}"),
            payload: "{}".into(),
            idempotency_key: None,
            privacy_class: Some("private".into()),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            priority: 128,
            task_kind: None,
            model_id: None,
            traceparent: None,
        })
        .await
        .unwrap();
    }
    let first_page = http
        .relay_a2a_inbox_limited("10", Some(2), None)
        .await
        .unwrap();
    assert_eq!(first_page.messages.len(), 2);
    let cursor = first_page.messages.last().unwrap().id;
    let page = http
        .relay_a2a_inbox_limited("10", Some(64), Some(cursor))
        .await
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    assert!(page.messages.iter().all(|m| m.id < cursor));

    server.abort();
}

#[tokio::test]
#[serial]
async fn a2a_inbox_all_paged_collects_full_inbox() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    for i in 0..5 {
        http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "11".into(),
            message_type: format!("m{i}"),
            payload: "{}".into(),
            idempotency_key: None,
            privacy_class: Some("private".into()),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            priority: 128,
            task_kind: None,
            model_id: None,
            traceparent: None,
        })
        .await
        .unwrap();
    }

    let all = http.relay_a2a_inbox_all_paged("11", 2).await.unwrap();
    assert_eq!(all.len(), 5);
    assert!(all.windows(2).all(|w| w[0].id > w[1].id));

    server.abort();
}

#[tokio::test]
#[serial]
async fn a2a_inbox_pager_next_page_walks_until_empty() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);

    for i in 0..4 {
        http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "12".into(),
            message_type: format!("m{i}"),
            payload: "{}".into(),
            idempotency_key: None,
            privacy_class: Some("private".into()),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            priority: 128,
            task_kind: None,
            model_id: None,
            traceparent: None,
        })
        .await
        .unwrap();
    }

    let mut pager = vox_populi::http_client::A2AInboxPager::new("12", 2);
    let p1 = pager.next_page(&http).await.unwrap();
    let p2 = pager.next_page(&http).await.unwrap();
    let p3 = pager.next_page(&http).await.unwrap();
    let p4 = pager.next_page(&http).await.unwrap();
    assert_eq!(p1.len(), 2);
    assert_eq!(p2.len(), 2);
    assert!(p3.is_empty());
    assert!(p4.is_empty());
    assert!(p1[0].id > p1[1].id);
    assert!(p2[0].id > p2[1].id);
    assert!(p1[1].id > p2[0].id);

    server.abort();
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn exec_lease_store_survives_restart_when_path_configured() {
    let _guard = ENV_MUTEX.lock().expect("env lock");
    let prev = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshExecLeaseStorePath)
        .expose()
        .map(|s| s.to_string());
    let dir = tempfile::tempdir().expect("tempdir");
    let lease_path = dir.path().join("exec-leases.json");
    unsafe {
        std::env::set_var("VOX_MESH_EXEC_LEASE_STORE_PATH", &lease_path);
    }

    let lease_id = {
        let state = PopuliTransportState::new_for_serve();
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        let bound = listener.local_addr().unwrap();
        let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let base = format!("http://{}", bound);
        wait_populi_http_ready(&base).await;

        let http = PopuliHttpClient::new(&base);
        http.join(&node_record_for_current_process(
            "persist-worker".into(),
            None,
        ))
        .await
        .unwrap();
        let grant = http
            .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
                claimer_node_id: "persist-worker".into(),
                scope_key: "task:persist-1".into(),
            })
            .await
            .unwrap();
        server.abort();
        grant.lease_id
    };

    let state2 = PopuliTransportState::new_for_serve();
    let addr2 = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener2 = tokio::net::TcpListener::bind(addr2).await.unwrap();
    let bound2 = listener2.local_addr().unwrap();
    let app2 = transport::populi_http_app_with_auth(state2, PopuliHttpAuth::Open);
    let server2 = tokio::spawn(async move {
        axum::serve(listener2, app2).await.unwrap();
    });
    let base2 = format!("http://{}", bound2);
    wait_populi_http_ready(&base2).await;

    let http2 = PopuliHttpClient::new(&base2);
    http2
        .join(&node_record_for_current_process(
            "persist-worker".into(),
            None,
        ))
        .await
        .unwrap();
    http2
        .exec_lease_renew(&vox_populi::transport::RemoteExecLeaseRenewRequest {
            lease_id,
            claimer_node_id: "persist-worker".into(),
        })
        .await
        .unwrap();
    server2.abort();

    unsafe {
        match prev {
            Some(v) => std::env::set_var("VOX_MESH_EXEC_LEASE_STORE_PATH", v),
            None => std::env::remove_var("VOX_MESH_EXEC_LEASE_STORE_PATH"),
        }
    }
}

#[tokio::test]
#[serial]
async fn mesh_jwt_hs256_accepts_and_rejects_jti_replay() {
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims<'a> {
        role: &'a str,
        jti: &'a str,
        exp: u64,
    }

    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let jwt_rt = transport::PopuliMeshAuthRuntime::with_jwt_hmac_only("jwt-unit-secret-for-test");
    let app =
        transport::populi_http_app_with_auth(state, transport::PopuliHttpAuth::Custom(jwt_rt));
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let token = encode(
        &Header::new(Algorithm::HS256),
        &Claims {
            role: "worker",
            jti: "jti-unit-once",
            exp,
        },
        &EncodingKey::from_secret(b"jwt-unit-secret-for-test"),
    )
    .unwrap();
    let url = format!("{base}/v1/populi/nodes");
    let ok = reqwest::Client::new()
        .get(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), reqwest::StatusCode::OK);
    let replay = reqwest::Client::new()
        .get(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), reqwest::StatusCode::UNAUTHORIZED);
    server.abort();
}

#[tokio::test]
#[serial]
async fn job_result_attestation_requires_full_key_when_fields_present() {
    let state = PopuliTransportState::new();
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let err = http
        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "1".into(),
            receiver_agent_id: "2".into(),
            message_type: vox_populi::transport::A2A_MESSAGE_JOB_RESULT.into(),
            payload: "{}".into(),
            idempotency_key: None,
            privacy_class: Some("private".into()),
            payload_blake3_hex: Some(
                "0000000000000000000000000000000000000000000000000000000000000000".into(),
            ),
            worker_ed25519_sig_b64: Some(
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            ),
            jwe_payload: None,
            priority: 128,
            task_kind: None,
            model_id: None,
            traceparent: None,
        })
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("503")
            || err.to_string().to_ascii_lowercase().contains("unavailable"),
        "{err}"
    );
    server.abort();
}

#[tokio::test]
#[serial]
async fn job_result_attestation_accepts_valid_signature() {
    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::RngCore;
    use rand::rngs::OsRng;

    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    let vk = sk.verifying_key();

    let state = PopuliTransportState::new().with_worker_result_verify_key(Some(vk.to_bytes()));
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let bound = listener.local_addr().unwrap();
    let app = transport::populi_http_app_with_auth(state, PopuliHttpAuth::Open);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let base = format!("http://{}", bound);
    wait_populi_http_ready(&base).await;

    let http = PopuliHttpClient::new(&base);
    let payload = r#"{"status":"ok"}"#;
    let digest = *blake3::hash(payload.as_bytes()).as_bytes();
    let sig = sk.sign(&digest);
    let digest_hex = data_encoding::HEXLOWER.encode(&digest);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "1".into(),
        receiver_agent_id: "2".into(),
        message_type: vox_populi::transport::A2A_MESSAGE_JOB_RESULT.into(),
        payload: payload.into(),
        idempotency_key: None,
        privacy_class: Some("private".into()),
        payload_blake3_hex: Some(digest_hex),
        worker_ed25519_sig_b64: Some(sig_b64),
        jwe_payload: None,
        priority: 128,
        task_kind: None,
        model_id: None,
        traceparent: None,
    })
    .await
    .unwrap();
    server.abort();
}
