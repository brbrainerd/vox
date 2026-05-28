//! HTTP relay for A2A over Populi mesh transport.
//!
//! P0-T3: every path that falls back to local execution after a failed relay
//! must call [`gate_local_fallback`] first. This enforces the W1 invariant —
//! a remote node holding an unexpired lease for the same scope blocks the
//! local fallback rather than duplicating execution.
//!
//! P6-T9: `HopperOpSync` message kinds are routed through the federation
//! envelope layer. See `crate::hopper::mesh_adapter` for the type definitions.

use crate::types::{A2AMessageType, AgentId, CompletionAttestation};

use super::super::envelope::{
    REMOTE_TASK_CANCEL_TYPE, REMOTE_TASK_ENVELOPE_TYPE, REMOTE_TASK_RESULT_TYPE, RemoteTaskCancel,
    RemoteTaskEnvelope, RemoteTaskResult,
};

/// Gate a local-fallback decision against the `mesh_exec_leases` table.
///
/// Call this after a mesh relay fails, before falling through to local execution.
/// Returns `Ok(())` when this node may proceed; returns `Err` when a remote node
/// holds an unexpired lease for `scope_key` (W1 guard, ADR-017).
/// Fails closed on DB error — returns `Err` rather than silently permitting.
pub async fn gate_local_fallback(
    db: &vox_db::VoxDb,
    scope_key: &str,
    self_node_id: &str,
) -> Result<(), super::lease_gate::LeaseGateError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    match super::lease_gate::check_before_local_fallback(db, scope_key, self_node_id, now).await {
        Ok(()) => Ok(()),
        Err(super::lease_gate::LeaseGateError::HeldByRemote { holder_node_id, .. }) => {
            tracing::warn!(
                scope_key = scope_key,
                holder = %holder_node_id,
                "gate_local_fallback: remote lease holder owns scope; refusing duplicate-execute"
            );
            Err(super::lease_gate::LeaseGateError::HeldByRemote {
                scope_key: scope_key.to_string(),
                holder_node_id,
                expires_at: 0,
            })
        }
        Err(e) => {
            tracing::warn!(error = %e, "lease_gate db error; failing closed");
            Err(e)
        }
    }
}

fn fnv1a64(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn default_mesh_idempotency_key(
    sender: AgentId,
    receiver: AgentId,
    msg_type: &A2AMessageType,
    payload: &str,
) -> String {
    format!(
        "mesh-{sender}-{receiver}-{:016x}",
        fnv1a64(&[&msg_type.to_string(), payload])
    )
}

/// Relay a message to another mens node via HTTP.
pub async fn relay_to_mesh(
    client: &vox_populi::http_client::PopuliHttpClient,
    sender: AgentId,
    receiver: AgentId,
    msg_type: A2AMessageType,
    payload: impl Into<String>,
) -> Result<(), String> {
    let payload = payload.into();
    let idempotency_key = default_mesh_idempotency_key(sender, receiver, &msg_type, &payload);
    client
        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: sender.0.to_string(),
            receiver_agent_id: receiver.0.to_string(),
            message_type: msg_type.to_string(),
            payload,
            idempotency_key: Some(idempotency_key),
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
        .map_err(|e: vox_populi::PopuliRegistryError| e.to_string())
}

/// AI-first `@subagent(policy = distributed)` fixture hook: best-effort mesh relay after local dispatch.
///
/// When [`vox_populi::http_lifecycle::populi_http_control_base_from_env`] resolves a control-plane URL,
/// sends a small [`A2AMessageType::HelpRequest`] tagging the dispatch decision. Otherwise returns
/// `{decision}|mesh=skipped_no_control_url` so fixture crates compile and run without Populi.
pub async fn relay_ai_fixture_distributed_subagent(
    dispatch_decision: &str,
    prompt_len: usize,
) -> String {
    let Some(base) = vox_populi::http_lifecycle::populi_http_control_base_from_env() else {
        return format!("{dispatch_decision}|mesh=skipped_no_control_url");
    };
    let timeout_ms = vox_populi::http_lifecycle::populi_http_timeout_ms_from_env();
    let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
        &base,
        std::time::Duration::from_millis(timeout_ms),
    )
    .with_env_deliver_token();
    let sender = AgentId(1);
    let receiver = AgentId(2);
    let payload =
        format!("ai_fixture_subagent dispatch={dispatch_decision} prompt_len={prompt_len}");
    match relay_to_mesh(
        &client,
        sender,
        receiver,
        A2AMessageType::HelpRequest,
        payload,
    )
    .await
    {
        Ok(()) => format!("{dispatch_decision}|mesh=relayed"),
        Err(e) => format!("{dispatch_decision}|mesh=error:{e}"),
    }
}

/// Relay a structured remote task envelope over the mesh A2A transport.
pub async fn relay_remote_task_envelope(
    client: &vox_populi::http_client::PopuliHttpClient,
    sender: AgentId,
    receiver: AgentId,
    envelope: &RemoteTaskEnvelope,
) -> Result<(), String> {
    let payload = serde_json::to_string(envelope).map_err(|e| e.to_string())?;

    let mut jwe_payload = None;
    if let Ok(reqs) =
        serde_json::from_str::<serde_json::Value>(&envelope.capability_requirements_json)
    {
        if let Some(arr) = reqs.get("required_secrets").and_then(|v| v.as_array()) {
            let mut resolved_map = std::collections::HashMap::new();
            for sec in arr {
                if let Some(sec_str) = sec.as_str() {
                    if let Ok(id) = sec_str.parse::<vox_secrets::spec::SecretId>() {
                        let res = vox_secrets::resolve_secret(id);
                        if let Some(val) = res.expose() {
                            resolved_map.insert(sec_str.to_string(), val.to_string());
                        }
                    }
                }
            }
            if !resolved_map.is_empty() {
                let mesh_secret_res =
                    vox_secrets::resolve_secret(vox_secrets::spec::SecretId::VoxMeshJwtHmacSecret);
                if let Some(mesh_val) = mesh_secret_res.expose() {
                    let derived = blake3::hash(mesh_val.as_bytes());
                    if let Ok(secret_json) = serde_json::to_string(&resolved_map) {
                        if let Ok(enc) = crate::a2a::jwe::encrypt_jwe_compact(
                            secret_json.as_bytes(),
                            derived.as_bytes(),
                        ) {
                            jwe_payload = Some(enc);
                        }
                    }
                }
            }
        }
    }

    client
        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: sender.0.to_string(),
            receiver_agent_id: receiver.0.to_string(),
            message_type: REMOTE_TASK_ENVELOPE_TYPE.to_string(),
            payload,
            idempotency_key: Some(envelope.idempotency_key.clone()),
            privacy_class: envelope.privacy_class.clone(),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload,
            task_kind: Some("vox_script".to_string()),
            model_id: None,
            traceparent: Some(crate::a2a::traceparent::encode(
                &crate::a2a::traceparent::TraceContext::from_current_span(),
            )),
            priority: 128,
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Relay a remote-task cancel hint to the mesh receiver (best-effort).
pub async fn relay_remote_task_cancel(
    client: &vox_populi::http_client::PopuliHttpClient,
    sender: AgentId,
    receiver: AgentId,
    cancel: &RemoteTaskCancel,
) -> Result<(), String> {
    let payload = serde_json::to_string(cancel).map_err(|e| e.to_string())?;
    let idempotency_key = format!(
        "orch-remote-cancel-{}-{}",
        cancel.task_id, cancel.idempotency_key
    );
    client
        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: sender.0.to_string(),
            receiver_agent_id: receiver.0.to_string(),
            message_type: REMOTE_TASK_CANCEL_TYPE.to_string(),
            payload,
            idempotency_key: Some(idempotency_key),
            privacy_class: None,
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            task_kind: None,
            model_id: None,
            traceparent: None,
            priority: 255,
        })
        .await
        .map(|_| ())
        .map_err(|e: vox_populi::PopuliRegistryError| e.to_string())
}

fn task_id_from_remote_mesh_idempotency(key: &str) -> Option<u64> {
    let rest = key.strip_prefix("orch-remote-")?;
    let (tid, _) = rest.split_once('-')?;
    tid.parse().ok()
}

/// Poll the populi HTTP inbox for [`REMOTE_TASK_RESULT_TYPE`] rows and complete/fail local tasks.
///
/// `parent_inbox_agent_id` must match the **sender** agent id used when relaying
/// [`RemoteTaskEnvelope`] so result deliveries addressed to the parent are visible here.
pub async fn drain_populi_remote_task_results(
    client: &vox_populi::http_client::PopuliHttpClient,
    parent_inbox_agent_id: u64,
    max_messages: usize,
    orchestrator: &crate::Orchestrator,
) {
    // Walk newest-first inbox pages so deep `remote_task_result` rows are not stuck behind unrelated mail.
    const MAX_PAGES: usize = 32;

    let mut budget = max_messages.max(1);
    let page_size = max_messages.max(1);
    let mut before_message_id: Option<u64> = None;

    'pages: for _ in 0..MAX_PAGES {
        if budget == 0 {
            break 'pages;
        }
        let Ok(inbox) = client
            .relay_a2a_inbox_limited(
                &parent_inbox_agent_id.to_string(),
                Some(page_size),
                before_message_id,
            )
            .await
        else {
            tracing::debug!(
                parent_inbox_agent_id = parent_inbox_agent_id,
                "populi remote result poll: inbox HTTP failed"
            );
            return;
        };

        if inbox.messages.is_empty() {
            break;
        }

        let page_len = inbox.messages.len();
        let next_cursor = inbox.messages.last().map(|m| m.id);

        for msg in inbox.messages.into_iter().take(page_size) {
            if budget == 0 {
                break 'pages;
            }
            if msg.message_type != REMOTE_TASK_RESULT_TYPE {
                continue;
            }
            let Ok(result) = serde_json::from_str::<RemoteTaskResult>(&msg.payload) else {
                tracing::debug!(
                    message_id = msg.id,
                    "populi remote result: skip invalid JSON payload"
                );
                continue;
            };
            let task_id = result
                .task_id
                .or_else(|| task_id_from_remote_mesh_idempotency(&result.idempotency_key));
            let Some(tid) = task_id else {
                tracing::debug!(
                    idempotency = %result.idempotency_key,
                    "populi remote result: could not resolve task id"
                );
                continue;
            };
            let task_id = crate::types::TaskId(tid);
            let delegate = orchestrator
                .agent_assigned_to_task(task_id)
                .and_then(|aid| orchestrator.agent_queue(aid))
                .and_then(|ql| {
                    let q = crate::sync_lock::rw_read(&*ql);
                    q.current_task()
                        .filter(|t| t.id == task_id)
                        .and_then(|t| t.populi_remote_delegate.clone())
                });
            if let Some(delegate) = delegate.as_ref() {
                if delegate.idempotency_key != result.idempotency_key {
                    tracing::debug!(
                        task_id = tid,
                        message_id = msg.id,
                        expected_idempotency = %delegate.idempotency_key,
                        received_idempotency = %result.idempotency_key,
                        "populi remote result: stale idempotency for task; acking row"
                    );
                    let _ = client
                        .relay_a2a_ack(&parent_inbox_agent_id.to_string(), msg.id)
                        .await;
                    budget = budget.saturating_sub(1);
                    continue;
                }
            }
            let terminal_res = if result.success {
                orchestrator
                    .complete_task_with_attestation(
                        task_id,
                        Some(CompletionAttestation {
                            checks_passed: vec!["peer_review_approved".to_string()],
                            ..Default::default()
                        }),
                    )
                    .await
            } else {
                let detail = result
                    .error
                    .clone()
                    .or(result.result.clone())
                    .unwrap_or_else(|| "remote task failed".to_string());
                orchestrator.fail_task(task_id, detail).await
            };

            let should_ack = matches!(
                &terminal_res,
                Ok(()) | Err(crate::orchestrator::OrchestratorError::TaskNotFound(_))
            );

            match &terminal_res {
                Ok(()) => {}
                Err(crate::orchestrator::OrchestratorError::TaskNotFound(_)) => {
                    tracing::debug!(
                        task_id = tid,
                        message_id = msg.id,
                        "populi remote result: task missing locally; acking stale inbox row"
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        error = %e,
                        task_id = tid,
                        message_id = msg.id,
                        "populi remote result: terminal transition failed; leaving row for retry"
                    );
                }
            }

            if should_ack {
                if let Err(e) = client
                    .relay_a2a_ack(&parent_inbox_agent_id.to_string(), msg.id)
                    .await
                {
                    tracing::debug!(
                        error = %e,
                        message_id = msg.id,
                        "populi remote result: ack failed"
                    );
                }
                budget = budget.saturating_sub(1);
                if let Some(delegate) = delegate
                    && let (Some(lease_id), Some(claimer_node_id)) =
                        (delegate.lease_id, delegate.claimer_node_id)
                {
                    let _ = client
                        .exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
                            lease_id,
                            claimer_node_id,
                        })
                        .await;
                }
            }
        }

        before_message_id = next_cursor;
        if before_message_id.is_none() || page_len < page_size {
            break;
        }
    }
}
