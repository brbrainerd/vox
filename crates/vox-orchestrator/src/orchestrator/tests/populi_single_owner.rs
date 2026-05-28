use super::*;
#[cfg(feature = "populi-transport")]
use crate::a2a::populi_remote_worker_tick_once;
use crate::config::OrchestratorConfig;
use crate::reconstruction::AgentExecutionRole;
use crate::types::{
    AgentTask, FileAffinity, PopuliRemoteDelegate, TaskEnqueueHints, TaskId, TaskPriority,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lease_gated_relay_failure_falls_back_to_local_queue() {
    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some("http://127.0.0.1:9".to_string());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");
    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        ..Default::default()
    };
    let tid = orch
        .submit_task_with_agent(
            "leased-class task",
            vec![],
            None,
            None,
            None,
            Some(hints),
            None,
            None,
        )
        .await
        .expect("submit");
    let aid = *orch
        .task_assignments
        .read()
        .unwrap()
        .get(&tid)
        .expect("assignment");
    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();
    assert!(
        !q.has_in_progress(),
        "relay failure must not leave a remote hold in progress"
    );
    assert_eq!(q.len(), 1, "task should be queued for local execution");
    let t = q.tasks().iter().find(|t| t.id == tid).expect("task");
    assert!(
        t.populi_remote_delegate.is_none(),
        "fallback task must not carry remote delegate"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn populi_remote_hold_completes_via_complete_task() {
    let orch = Orchestrator::new(OrchestratorConfig::for_testing());
    orch.spawn_agent("solo").expect("spawn");
    let aid = orch.agent_ids()[0];
    let mut task = AgentTask::new(TaskId(901), "remote-only", TaskPriority::Normal, vec![]);
    task.populi_remote_delegate = Some(PopuliRemoteDelegate {
        idempotency_key: "orch-remote-901-test".into(),
        lease_id: None,
        claimer_node_id: None,
    });
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = ql.write().unwrap();
        q.hold_for_populi_remote(task).expect("hold");
    }
    orch.task_assignments
        .write()
        .unwrap()
        .insert(TaskId(901), aid);
    orch.complete_task(TaskId(901))
        .await
        .expect("complete remote-held task");
    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();
    assert!(!q.has_in_progress());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_populi_remote_delegated_clears_assignment() {
    let orch = Orchestrator::new(OrchestratorConfig::for_testing());
    orch.spawn_agent("solo").expect("spawn");
    let aid = orch.agent_ids()[0];
    let path = std::path::Path::new("populi_single_owner/cancel_test.rs");
    let mut task = AgentTask::new(
        TaskId(902),
        "cancel-remote",
        TaskPriority::Normal,
        vec![FileAffinity::write(path)],
    );
    task.populi_remote_delegate = Some(PopuliRemoteDelegate {
        idempotency_key: "k902".into(),
        lease_id: None,
        claimer_node_id: None,
    });
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = ql.write().unwrap();
        q.hold_for_populi_remote(task).expect("hold");
    }
    let _ = orch
        .lock_manager
        .try_acquire(path, aid, crate::locks::LockKind::Exclusive);
    orch.affinity_map.assign(path, aid);
    orch.task_assignments
        .write()
        .unwrap()
        .insert(TaskId(902), aid);
    orch.cancel_task(TaskId(902)).expect("cancel");
    assert!(
        !orch
            .task_assignments
            .read()
            .unwrap()
            .contains_key(&TaskId(902))
    );
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lease_gated_submit_holds_then_completes_via_populi_result_poll() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];

    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");
    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        ..Default::default()
    };
    let tid = orch
        .submit_task_with_agent(
            "leased-e2e task",
            vec![],
            None,
            None,
            None,
            Some(hints),
            None,
            None,
        )
        .await
        .expect("submit");
    let aid = *orch
        .task_assignments
        .read()
        .unwrap()
        .get(&tid)
        .expect("assignment");

    // Lease-gated path holds remote in progress, not local queue.
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let q = ql.read().unwrap();
        assert!(q.has_in_progress(), "expected remote-held in-progress task");
        assert_eq!(q.len(), 0, "expected no local queued copy");
    }
    let delegate_idempotency = {
        let ql = orch.agent_queue(aid).expect("queue");
        let q = ql.read().unwrap();
        q.current_task()
            .and_then(|t| t.populi_remote_delegate.as_ref())
            .map(|d| d.idempotency_key.clone())
            .expect("delegate idempotency")
    };

    let payload = serde_json::to_string(&RemoteTaskResult {
        idempotency_key: delegate_idempotency,
        success: false,
        result: None,
        error: Some("remote execution failed (test)".to_string()),
        task_id: Some(tid.0),
    })
    .expect("serialize remote result");
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "2".into(),
        receiver_agent_id: "1".into(),
        message_type: REMOTE_TASK_RESULT_TYPE.to_string(),
        payload,
        idempotency_key: Some(format!("remote-result-{}", tid.0)),
        privacy_class: None,
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        task_kind: None,
        model_id: None,
        traceparent: None,
        priority: 128,
    })
    .await
    .expect("relay result row");

    let mut cleared = false;
    for _ in 0..10 {
        crate::a2a::populi_remote_result_poll_once(&orch).await;
        let ql = orch.agent_queue(aid).expect("queue");
        let q = ql.read().unwrap();
        if !q.has_in_progress() {
            cleared = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        cleared,
        "in-progress slot should be cleared after remote completion"
    );
    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();
    assert!(
        !q.has_in_progress(),
        "in-progress slot should be cleared after remote completion"
    );
    let inbox_after = http.relay_a2a_inbox("1").await.expect("inbox after");
    assert!(
        inbox_after
            .messages
            .iter()
            .all(|m| m.message_type != REMOTE_TASK_RESULT_TYPE),
        "remote_task_result row should be acked after terminal transition"
    );

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lease_gated_submit_relays_context_envelope_in_payload() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");

    let sid = "lease-payload-session";
    let retrieval = crate::SessionRetrievalEnvelope {
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 2,
        knowledge_hit_count: 1,
        chunk_hit_count: 0,
        repo_hit_count: 0,
        rrf_fused_hit_count: 1,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 0,
        source_diversity: 2,
        evidence_quality: 0.8,
        citation_coverage: 0.9,
        verification_performed: false,
        verification_reason: None,
        recommended_next_action: None,
    };
    let context = crate::ContextEnvelope::from_session_retrieval("repo-test", sid, &retrieval);
    let context_json = serde_json::to_string(&context).expect("serialize context envelope");
    let key = crate::socrates::session_context_envelope_key(sid);
    crate::sync_lock::rw_write(&*orch.context_store).set(
        crate::types::AgentId(0),
        key,
        &context_json,
        3600,
    );
    let harness = crate::AgentHarnessSpec::minimal_contract_first(
        "repo-test",
        "leased-context-payload",
        Some(sid),
        Some("thread-lease"),
        &["artifacts/result.md".to_string()],
    );
    let harness_json = serde_json::to_string(&harness).expect("serialize harness");

    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        thread_id: Some("thread-lease".to_string()),
        harness_spec_json: Some(harness_json.clone()),
        ..Default::default()
    };
    let _tid = orch
        .submit_task_with_agent(
            "leased-context-payload",
            vec![],
            None,
            None,
            None,
            Some(hints),
            Some(sid.to_string()),
            None,
        )
        .await
        .expect("submit");

    let mut relayed_payload: Option<serde_json::Value> = None;
    for _ in 0..20 {
        let inbox = http.relay_a2a_inbox("2").await.expect("inbox");
        if let Some(msg) = inbox
            .messages
            .iter()
            .find(|m| m.message_type == REMOTE_TASK_ENVELOPE_TYPE)
        {
            let env: RemoteTaskEnvelope =
                serde_json::from_str(&msg.payload).expect("remote envelope parse");
            relayed_payload = Some(
                serde_json::from_str::<serde_json::Value>(&env.payload).expect("payload parse"),
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let payload = relayed_payload.expect("expected remote_task_envelope relay");
    assert_eq!(payload["session_id"], serde_json::json!(sid));
    assert_eq!(payload["thread_id"], serde_json::json!("thread-lease"));
    assert_eq!(
        payload["context_envelope_json"],
        serde_json::json!(context_json)
    );
    assert_eq!(
        payload["harness_spec_json"],
        serde_json::json!(harness_json)
    );

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_worker_tick_once_seeds_context_and_attaches_socrates_when_task_assigned() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_worker_poll_interval_secs = 1;
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");
    let aid = orch.agent_ids()[0];

    let remote_task_id = TaskId(9944);
    let mut task = AgentTask::new(
        remote_task_id,
        "remote-worker-seed",
        TaskPriority::Normal,
        vec![],
    );
    task.populi_remote_delegate = Some(PopuliRemoteDelegate {
        idempotency_key: "k9944".into(),
        lease_id: None,
        claimer_node_id: None,
    });
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = ql.write().unwrap();
        q.hold_for_populi_remote(task).expect("hold");
    }
    orch.task_assignments
        .write()
        .unwrap()
        .insert(remote_task_id, aid);

    let sid = "worker-seed-session";
    let retrieval = crate::SessionRetrievalEnvelope {
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 2,
        knowledge_hit_count: 1,
        chunk_hit_count: 0,
        repo_hit_count: 0,
        rrf_fused_hit_count: 1,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 0,
        source_diversity: 2,
        evidence_quality: 0.8,
        citation_coverage: 0.9,
        verification_performed: false,
        verification_reason: None,
        recommended_next_action: None,
    };
    let context =
        crate::ContextEnvelope::from_session_retrieval("repo-worker-test", sid, &retrieval);
    let context_json = serde_json::to_string(&context).expect("serialize context envelope");
    let key = crate::socrates::session_context_envelope_key(sid);
    assert!(
        crate::sync_lock::rw_read(&*orch.context_store)
            .get(&key)
            .is_none(),
        "context key should not be pre-seeded"
    );

    let inner_payload = serde_json::json!({
        "task_description": "remote worker task",
        "assigned_agent_id": aid.0,
        "session_id": sid,
        "context_envelope_json": context_json,
    })
    .to_string();
    let envelope = RemoteTaskEnvelope {
        idempotency_key: "remote-worker-9944".to_string(),
        task_id: remote_task_id.0,
        repository_id: "repo-worker-test".to_string(),
        capability_requirements_json: "{}".to_string(),
        payload: inner_payload,
        privacy_class: None,
        populi_scope_id: None,
        submitted_unix_ms: Some(crate::types::now_unix_ms()),
        exec_lease_id: Some("orchestrator-lease".to_string()),
        campaign_id: None,
        artifact_refs_json: None,
        session_id: Some(sid.to_string()),
        thread_id: None,
        context_envelope_json: Some(serde_json::to_string(&context).expect("serialize context")),
        harness_spec_json: None,
        parent_task_id: None,
        caller_agent_id: None,
        trace_id: None,
        span_depth: None,
    };
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "1".into(),
        receiver_agent_id: "2".into(),
        message_type: REMOTE_TASK_ENVELOPE_TYPE.to_string(),
        payload: serde_json::to_string(&envelope).expect("serialize envelope"),
        idempotency_key: Some("inject-remote-worker-9944".to_string()),
        privacy_class: None,
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        task_kind: None,
        model_id: None,
        traceparent: None,
        priority: 128,
    })
    .await
    .expect("deliver remote envelope");

    populi_remote_worker_tick_once(&orch).await;

    let stored = crate::sync_lock::rw_read(&*orch.context_store)
        .get(&key)
        .expect("worker should seed context envelope key");
    assert_eq!(stored, serde_json::to_string(&context).expect("serialize"));

    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();
    let soc = q
        .current_task()
        .and_then(|t| t.socrates.as_ref())
        .expect("worker should attach Socrates context when task is assigned");
    assert_eq!(soc.retrieval_tier.as_deref(), Some("hybrid"));

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_worker_tick_once_accepts_object_context_envelope_payload() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_worker_poll_interval_secs = 1;
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");

    let sid = "worker-object-context-session";
    let retrieval = crate::SessionRetrievalEnvelope {
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 1,
        knowledge_hit_count: 1,
        chunk_hit_count: 0,
        repo_hit_count: 0,
        rrf_fused_hit_count: 0,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 0,
        source_diversity: 2,
        evidence_quality: 0.7,
        citation_coverage: 0.8,
        verification_performed: false,
        verification_reason: None,
        recommended_next_action: None,
    };
    let context =
        crate::ContextEnvelope::from_session_retrieval("repo-object-worker", sid, &retrieval);
    let context_value = serde_json::to_value(&context).expect("serialize context");
    let key = crate::socrates::session_context_envelope_key(sid);
    assert!(
        crate::sync_lock::rw_read(&*orch.context_store)
            .get(&key)
            .is_none(),
        "context key should not be pre-seeded"
    );

    let inner_payload = serde_json::json!({
        "task_description": "remote worker object payload",
        "session_id": sid,
        "context_envelope_json": context_value,
    })
    .to_string();
    let envelope = RemoteTaskEnvelope {
        idempotency_key: "remote-worker-object-9955".to_string(),
        task_id: 9955,
        repository_id: "repo-object-worker".to_string(),
        capability_requirements_json: "{}".to_string(),
        payload: inner_payload,
        privacy_class: None,
        populi_scope_id: None,
        submitted_unix_ms: Some(crate::types::now_unix_ms()),
        exec_lease_id: Some("orchestrator-lease".to_string()),
        campaign_id: None,
        artifact_refs_json: None,
        session_id: Some(sid.to_string()),
        thread_id: None,
        context_envelope_json: Some(serde_json::to_string(&context).expect("serialize context")),
        harness_spec_json: None,
        parent_task_id: None,
        caller_agent_id: None,
        trace_id: None,
        span_depth: None,
    };
    http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
        sender_agent_id: "1".into(),
        receiver_agent_id: "2".into(),
        message_type: REMOTE_TASK_ENVELOPE_TYPE.to_string(),
        payload: serde_json::to_string(&envelope).expect("serialize envelope"),
        idempotency_key: Some("inject-remote-worker-object-9955".to_string()),
        privacy_class: None,
        payload_blake3_hex: None,
        worker_ed25519_sig_b64: None,
        jwe_payload: None,
        task_kind: None,
        model_id: None,
        traceparent: None,
        priority: 128,
    })
    .await
    .expect("deliver remote envelope");

    populi_remote_worker_tick_once(&orch).await;

    let stored = crate::sync_lock::rw_read(&*orch.context_store)
        .get(&key)
        .expect("worker should seed context envelope key");
    let parsed: crate::ContextEnvelope =
        serde_json::from_str(&stored).expect("stored context json");
    assert_eq!(
        parsed.envelope_type,
        crate::ContextEnvelopeType::RetrievalEvidence
    );

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_populi_remote_delegated_relays_remote_cancel_message() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base);
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("solo").expect("spawn");
    let aid = orch.agent_ids()[0];

    let mut task = AgentTask::new(
        TaskId(9902),
        "cancel-remote-net",
        TaskPriority::Normal,
        vec![],
    );
    task.populi_remote_delegate = Some(PopuliRemoteDelegate {
        idempotency_key: "k9902".into(),
        lease_id: None,
        claimer_node_id: None,
    });
    {
        let ql = orch.agent_queue(aid).expect("queue");
        let mut q = ql.write().unwrap();
        q.hold_for_populi_remote(task).expect("hold");
    }
    orch.task_assignments
        .write()
        .unwrap()
        .insert(TaskId(9902), aid);
    orch.cancel_task(TaskId(9902)).expect("cancel");

    let mut saw_cancel = false;
    for _ in 0..20 {
        let inbox = http.relay_a2a_inbox("2").await.expect("inbox");
        if inbox
            .messages
            .iter()
            .any(|m| m.message_type == REMOTE_TASK_CANCEL_TYPE)
        {
            saw_cancel = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(saw_cancel, "expected remote_task_cancel delivery");

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lease_renew_loss_requeues_locally_and_relays_cancel() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");
    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        ..Default::default()
    };
    let tid = orch
        .submit_task_with_agent(
            "leased-renew-loss",
            vec![],
            None,
            None,
            None,
            Some(hints),
            None,
            None,
        )
        .await
        .expect("submit");
    let aid = *orch
        .task_assignments
        .read()
        .unwrap()
        .get(&tid)
        .expect("assignment");
    let delegate = {
        let ql = orch.agent_queue(aid).expect("queue");
        let q = ql.read().unwrap();
        q.current_task()
            .and_then(|t| t.populi_remote_delegate.clone())
            .expect("delegate")
    };
    let lease_id = delegate.lease_id.clone().expect("lease_id");
    let claimer_node_id = delegate.claimer_node_id.clone().expect("claimer");
    http.exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
        lease_id,
        claimer_node_id,
    })
    .await
    .expect("force lease loss");

    crate::a2a::populi_remote_result_poll_once(&orch).await;

    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();
    assert!(!q.has_in_progress(), "lease-loss should clear remote hold");
    let task = q
        .tasks()
        .iter()
        .find(|t| t.id == tid)
        .expect("requeued task");
    assert!(
        task.populi_remote_delegate.is_none(),
        "fallback requeue should remove remote delegate"
    );
    drop(q);

    let mut saw_cancel = false;
    for _ in 0..20 {
        let inbox = http.relay_a2a_inbox("2").await.expect("inbox");
        if inbox
            .messages
            .iter()
            .any(|m| m.message_type == REMOTE_TASK_CANCEL_TYPE)
        {
            saw_cancel = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(saw_cancel, "lease-loss fallback should relay cancel");

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_result_poll_respects_max_messages_per_poll() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);
    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_result_max_messages_per_poll = 1;
    let orch = Orchestrator::new(cfg);

    for i in 0..3u64 {
        let aid = orch.spawn_agent(&format!("w{i}")).expect("spawn");
        let tid = TaskId(12_000 + i);
        let mut task = AgentTask::new(tid, format!("held-{i}"), TaskPriority::Normal, vec![]);
        task.populi_remote_delegate = Some(PopuliRemoteDelegate {
            idempotency_key: format!("orch-remote-{}-t", tid.0),
            lease_id: None,
            claimer_node_id: None,
        });
        {
            let ql = orch.agent_queue(aid).expect("queue");
            let mut q = ql.write().unwrap();
            q.hold_for_populi_remote(task).expect("hold");
        }
        orch.task_assignments.write().unwrap().insert(tid, aid);
        let payload = serde_json::to_string(&RemoteTaskResult {
            idempotency_key: format!("orch-remote-{}-t", tid.0),
            success: false,
            result: None,
            error: Some("fail".to_string()),
            task_id: Some(tid.0),
        })
        .expect("serialize");
        http.relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: "2".into(),
            receiver_agent_id: "1".into(),
            message_type: REMOTE_TASK_RESULT_TYPE.to_string(),
            payload,
            idempotency_key: Some(format!("remote-result-{}", tid.0)),
            privacy_class: None,
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            task_kind: None,
            model_id: None,
            traceparent: None,
            priority: 128,
        })
        .await
        .expect("relay result");
    }

    crate::a2a::populi_remote_result_poll_once(&orch).await;
    let after_one = orch
        .agent_ids()
        .into_iter()
        .filter(|aid| {
            let ql = orch.agent_queue(*aid).expect("queue");
            ql.read().unwrap().has_in_progress()
        })
        .count();
    assert_eq!(
        after_one, 2,
        "max-per-poll=1 should clear one held task per tick"
    );
    let inbox_after_one = http.relay_a2a_inbox("1").await.expect("inbox");
    let remaining_after_one = inbox_after_one
        .messages
        .iter()
        .filter(|m| m.message_type == REMOTE_TASK_RESULT_TYPE)
        .count();
    assert_eq!(remaining_after_one, 2);

    crate::a2a::populi_remote_result_poll_once(&orch).await;
    crate::a2a::populi_remote_result_poll_once(&orch).await;
    let after_three = orch
        .agent_ids()
        .into_iter()
        .filter(|aid| {
            let ql = orch.agent_queue(*aid).expect("queue");
            ql.read().unwrap().has_in_progress()
        })
        .count();
    assert_eq!(
        after_three, 0,
        "all held tasks should clear after three polls"
    );
    let inbox_after_three = http.relay_a2a_inbox("1").await.expect("inbox");
    let remaining_after_three = inbox_after_three
        .messages
        .iter()
        .filter(|m| m.message_type == REMOTE_TASK_RESULT_TYPE)
        .count();
    assert_eq!(remaining_after_three, 0);

    server.abort();
}

#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn non_lease_remote_relay_includes_session_and_context_payload() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = false;
    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");

    let sid = "non-lease-relay-session";
    let retrieval = crate::SessionRetrievalEnvelope {
        retrieval_tier: "hybrid".to_string(),
        memory_hit_count: 1,
        knowledge_hit_count: 1,
        chunk_hit_count: 0,
        repo_hit_count: 0,
        rrf_fused_hit_count: 0,
        used_vector: true,
        used_bm25: true,
        used_lexical_fallback: false,
        contradiction_count: 0,
        source_diversity: 2,
        evidence_quality: 0.8,
        citation_coverage: 0.8,
        verification_performed: false,
        verification_reason: None,
        recommended_next_action: None,
    };
    let context = crate::ContextEnvelope::from_session_retrieval("repo-non-lease", sid, &retrieval);
    let context_json = serde_json::to_string(&context).expect("serialize context envelope");
    let key = crate::socrates::session_context_envelope_key(sid);
    crate::sync_lock::rw_write(&*orch.context_store).set(
        crate::types::AgentId(0),
        key,
        &context_json,
        3600,
    );

    let _tid = orch
        .submit_task_with_agent(
            "non-lease remote relay context",
            vec![],
            None,
            None,
            None,
            None,
            Some(sid.to_string()),
        )
        .await
        .expect("submit");

    let mut relayed_payload: Option<serde_json::Value> = None;
    for _ in 0..25 {
        let inbox = http.relay_a2a_inbox("2").await.expect("inbox");
        if let Some(msg) = inbox
            .messages
            .iter()
            .find(|m| m.message_type == REMOTE_TASK_ENVELOPE_TYPE)
        {
            let env: RemoteTaskEnvelope =
                serde_json::from_str(&msg.payload).expect("remote envelope parse");
            relayed_payload = Some(
                serde_json::from_str::<serde_json::Value>(&env.payload).expect("payload parse"),
            );
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let payload = relayed_payload.expect("expected non-lease relay payload");
    assert_eq!(payload["session_id"], serde_json::json!(sid));
    assert_eq!(
        payload["context_envelope_json"],
        serde_json::json!(context_json)
    );

    server.abort();
}

#[cfg(test)]
mod route_replay_tests {
    use super::*;
    use crate::groups::{AffinityGroup, AffinityGroupRegistry};
    use crate::types::{FileAffinity, PopuliRemoteDelegate};

    fn agent_id_named(orch: &Orchestrator, name: &str) -> crate::types::AgentId {
        let agents = orch.agents.read().unwrap();
        for (id, q) in agents.iter() {
            if q.read().unwrap().name == name {
                return *id;
            }
        }
        panic!("no agent named {name}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn replay_moves_queued_task_to_group_default_agent() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        orch.spawn_agent("heavy").expect("spawn heavy");
        let light_id = orch.spawn_agent("light").expect("spawn light");
        {
            let mut groups = orch.groups.write().unwrap();
            *groups = AffinityGroupRegistry::new(vec![AffinityGroup {
                name: "route-replay-fixture".to_string(),
                patterns: vec!["**/route_replay_fixture/**".to_string()],
                default_agent: Some(light_id),
            }]);
        }
        let path = FileAffinity::write("route_replay_fixture/task.rs");
        let tid = orch
            .submit_task_with_agent(
                "affinity replay",
                vec![path],
                None,
                Some("heavy".into()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit");
        let heavy_id = agent_id_named(&orch, "heavy");
        assert_eq!(
            *orch
                .task_assignments
                .read()
                .unwrap()
                .get(&tid)
                .expect("assignment"),
            heavy_id
        );

        let moved = orch
            .replay_queued_routes_after_populi_schedulable_drop()
            .await;
        assert!(
            moved >= 1,
            "expected route replay to move at least one queued task toward group default"
        );
        assert_eq!(
            *orch
                .task_assignments
                .read()
                .unwrap()
                .get(&tid)
                .expect("assignment after replay"),
            light_id
        );
        let q_heavy = orch.agent_queue(heavy_id).expect("heavy queue");
        assert!(
            !q_heavy.read().unwrap().tasks().iter().any(|t| t.id == tid),
            "task should leave heavy pending queue"
        );
        let q_light = orch.agent_queue(light_id).expect("light queue");
        assert!(
            q_light.read().unwrap().tasks().iter().any(|t| t.id == tid),
            "task should land on light"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn replay_skips_tasks_with_populi_remote_delegate() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        orch.spawn_agent("heavy").expect("spawn heavy");
        let light_id = orch.spawn_agent("light").expect("spawn light");
        {
            let mut groups = orch.groups.write().unwrap();
            *groups = AffinityGroupRegistry::new(vec![AffinityGroup {
                name: "route-replay-fixture".to_string(),
                patterns: vec!["**/route_replay_fixture/**".to_string()],
                default_agent: Some(light_id),
            }]);
        }
        let tid = orch
            .submit_task_with_agent(
                "delegate hold",
                vec![FileAffinity::write("route_replay_fixture/delegate_skip.rs")],
                None,
                Some("heavy".into()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("submit");
        let heavy_id = agent_id_named(&orch, "heavy");
        let mut task = {
            let q = orch.agent_queue(heavy_id).expect("heavy queue");
            let mut w = q.write().unwrap();
            w.cancel(tid).expect("task on heavy queue")
        };
        task.populi_remote_delegate = Some(PopuliRemoteDelegate {
            idempotency_key: "replay-delegate-skip".into(),
            lease_id: None,
            claimer_node_id: None,
        });
        {
            let q = orch.agent_queue(heavy_id).expect("heavy queue");
            q.write().unwrap().enqueue(task);
        }

        let moved = orch
            .replay_queued_routes_after_populi_schedulable_drop()
            .await;
        assert_eq!(moved, 0, "delegate tasks must not be replay-routed");
        assert_eq!(
            *orch
                .task_assignments
                .read()
                .unwrap()
                .get(&tid)
                .expect("assignment"),
            heavy_id
        );
        let q_heavy = orch.agent_queue(heavy_id).expect("heavy queue");
        assert!(
            q_heavy.read().unwrap().tasks().iter().any(|t| t.id == tid),
            "task stays pending on heavy"
        );
    }
}

// ── W2 admission control tests ────────────────────────────────────────────────

/// A task requiring 12 GB VRAM must fall back to local queue when the only
/// registered node advertises only 8 GB (S2 DoD: planner refuses to dispatch
/// a 12 GB job to a box with 8 GB free).
#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vram_admission_rejects_oversized_task_and_falls_back_to_local_queue() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    // Register a node that only has 8 GB VRAM.
    let mut node = vox_populi::NodeRecord {
        id: "worker-8gb".to_string(),
        capabilities: vox_repository::TaskCapabilityHints {
            gpu_cuda: true,
            min_vram_mb: Some(8192),
            ..Default::default()
        },
        listen_addr: None,
        version: "test".to_string(),
        last_seen_unix_ms: 0,
        scope_id: None,
        pool_id: None,
        trust_tier: None,
        workload_classes: None,
        privacy_class: None,
        loaded_llm_models: None,
        maintenance: None,
        maintenance_until_unix_ms: None,
        provider: None,
        gpu_total_count: None,
        gpu_healthy_count: None,
        gpu_allocatable_count: None,
        gpu_inventory_source: None,
        gpu_truth_layer: None,
        nvidia_driver_version: None,
        cuda_driver_version: None,
        gpu_readiness_ok: Some(true),
        gpu_readiness_reason: None,
        gpu_readiness_checked_unix_ms: None,
        quarantined: None,
        host_triple: None,
        cpu_usage_pct: None,
        memory_free_bytes: None,
        owner_vox_user_id: None,
        advertised_models: None,
        donation_policy: None,
        ed25519_pub_key_b64: None,
        probe_failures: None,
        visibility: None,
    };
    node.last_seen_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    http.join(&node).await.expect("join 8gb node");

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];

    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");

    // Submit a task that requires 12 GB VRAM — more than the 8 GB node offers.
    let cap = crate::contract::TaskCapabilityHints {
        gpu_cuda: true,
        min_vram_mb: Some(12288),
        ..Default::default()
    };
    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        ..Default::default()
    };
    let tid = orch
        .submit_task_with_agent(
            "12gb-task",
            vec![],
            None,
            None,
            Some(cap),
            Some(hints),
            None,
            None,
        )
        .await
        .expect("submit");

    let aid = *orch
        .task_assignments
        .read()
        .unwrap()
        .get(&tid)
        .expect("assignment");
    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();

    assert!(
        !q.has_in_progress(),
        "insufficient VRAM must not hold task in remote-progress"
    );
    assert_eq!(
        q.len(),
        1,
        "insufficient VRAM: task must fall back to local queue"
    );
    let t = q.tasks().iter().find(|t| t.id == tid).expect("task");
    assert!(
        t.populi_remote_delegate.is_none(),
        "fallback task must not carry remote delegate"
    );

    server.abort();
}

/// A task requiring 12 GB VRAM dispatches to remote hold when a node with
/// 16 GB is registered (positive path for W2 admission control).
#[cfg(feature = "populi-transport")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vram_admission_allows_task_when_node_meets_requirement() {
    let state = vox_populi::transport::PopuliTransportState::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind seed");
    let bound = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let app = vox_populi::transport::populi_http_app_with_auth(
            state,
            vox_populi::transport::PopuliHttpAuth::Open,
        );
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("serve");
    });

    let base = format!("http://{bound}");
    let http = vox_populi::http_client::PopuliHttpClient::new(&base);

    // Register a node that has 16 GB VRAM — sufficient for the 12 GB task.
    let mut node = vox_populi::NodeRecord {
        id: "worker-16gb".to_string(),
        capabilities: vox_repository::TaskCapabilityHints {
            gpu_cuda: true,
            min_vram_mb: Some(16384),
            ..Default::default()
        },
        listen_addr: None,
        version: "test".to_string(),
        last_seen_unix_ms: 0,
        scope_id: None,
        pool_id: None,
        trust_tier: None,
        workload_classes: None,
        privacy_class: None,
        loaded_llm_models: None,
        maintenance: None,
        maintenance_until_unix_ms: None,
        provider: None,
        gpu_total_count: None,
        gpu_healthy_count: None,
        gpu_allocatable_count: None,
        gpu_inventory_source: None,
        gpu_truth_layer: None,
        nvidia_driver_version: None,
        cuda_driver_version: None,
        gpu_readiness_ok: Some(true),
        gpu_readiness_reason: None,
        gpu_readiness_checked_unix_ms: None,
        quarantined: None,
        host_triple: None,
        cpu_usage_pct: None,
        memory_free_bytes: None,
        owner_vox_user_id: None,
        advertised_models: None,
        donation_policy: None,
        ed25519_pub_key_b64: None,
        probe_failures: None,
        visibility: None,
    };
    node.last_seen_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    http.join(&node).await.expect("join 16gb node");

    let mut cfg = OrchestratorConfig::for_testing();
    cfg.populi_remote_execute_experimental = true;
    cfg.populi_control_url = Some(base.clone());
    cfg.populi_remote_execute_receiver_agent = Some("2".to_string());
    cfg.populi_remote_execute_sender_agent = Some("1".to_string());
    cfg.populi_remote_lease_gating_enabled = true;
    cfg.populi_remote_lease_gated_roles = vec![AgentExecutionRole::Builder];

    let orch = Orchestrator::new(cfg);
    orch.spawn_agent("worker").expect("spawn");

    let cap = crate::contract::TaskCapabilityHints {
        gpu_cuda: true,
        min_vram_mb: Some(12288),
        ..Default::default()
    };
    let hints = TaskEnqueueHints {
        execution_role: Some(AgentExecutionRole::Builder),
        ..Default::default()
    };
    let tid = orch
        .submit_task_with_agent(
            "12gb-task-fits",
            vec![],
            None,
            None,
            Some(cap),
            Some(hints),
            None,
        )
        .await
        .expect("submit");

    let aid = *orch
        .task_assignments
        .read()
        .unwrap()
        .get(&tid)
        .expect("assignment");
    let ql = orch.agent_queue(aid).expect("queue");
    let q = ql.read().unwrap();

    assert!(
        q.has_in_progress(),
        "16gb node satisfies 12gb requirement: task must be held remote"
    );
    assert_eq!(q.len(), 0, "no local copy when remote hold is active");

    server.abort();
}
