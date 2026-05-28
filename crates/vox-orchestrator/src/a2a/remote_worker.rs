//! Background remote worker loop for Populi `remote_task_envelope` rows.

use std::sync::Arc;
use std::sync::Mutex;

use tracing::Instrument as _;

use super::envelope::{
    REMOTE_TASK_ENVELOPE_TYPE, REMOTE_TASK_RESULT_TYPE, RemoteTaskEnvelope, RemoteTaskResult,
};

#[derive(Debug, Default)]
struct RemotePayloadContext {
    session_id: Option<String>,
    thread_id: Option<String>,
    context_envelope_json: Option<String>,
    harness_spec_json: Option<String>,
}

fn parse_remote_payload_context(payload: &str) -> RemotePayloadContext {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return RemotePayloadContext::default();
    };
    let session_id = value
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);
    let thread_id = value
        .get("thread_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);
    let context_envelope_json = value.get("context_envelope_json").and_then(|v| {
        if let Some(s) = v.as_str() {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        } else if v.is_object() {
            serde_json::to_string(v).ok()
        } else {
            None
        }
    });
    let harness_spec_json = value.get("harness_spec_json").and_then(|v| {
        if let Some(s) = v.as_str() {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        } else if v.is_object() {
            serde_json::to_string(v).ok()
        } else {
            None
        }
    });
    RemotePayloadContext {
        session_id,
        thread_id,
        context_envelope_json,
        harness_spec_json,
    }
}

/// Process a single `remote_task_envelope` inbox row inside a tracing span.
///
/// The span records `vox.mesh.trace_id` from the W3C `traceparent` carried on
/// the inbox message (S1 level: field attachment only; full context propagation
/// is S2).
async fn process_one_envelope(
    orchestrator: &crate::orchestrator::Orchestrator,
    client: &vox_populi::http_client::PopuliHttpClient,
    sender_agent: u64,
    receiver_agent: u64,
    msg: vox_populi::transport::A2AStoredMessage,
    node_id: &str,
) {
    let envelope = match serde_json::from_str::<RemoteTaskEnvelope>(&msg.payload) {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(
                message_id = msg.id,
                error = %e,
                "populi remote worker: invalid envelope JSON; acking to drain poison-pill"
            );
            let _ = client
                .relay_a2a_ack(&receiver_agent.to_string(), msg.id)
                .await;
            return;
        }
    };

    // Parse the W3C traceparent into a structured context (P0-T8).
    let trace_ctx = msg
        .traceparent
        .as_deref()
        .and_then(crate::a2a::traceparent::parse);
    let trace_id = trace_ctx
        .as_ref()
        .map(|c| c.trace_id.as_str())
        .unwrap_or("");
    let parent_id = trace_ctx
        .as_ref()
        .map(|c| c.parent_id.as_str())
        .unwrap_or("");
    let exec_lease_id = envelope.exec_lease_id.as_deref().unwrap_or("");
    let span = tracing::info_span!(
        "populi_remote_envelope",
        task_id = envelope.task_id,
        message_id = msg.id,
        exec_lease_id,
        "vox.mesh.trace_id" = trace_id,
        "vox.mesh.parent_span_id" = parent_id,
    );

    async {
        tracing::info!("populi remote worker: processing envelope");

    // Decrypt JWE-wrapped secrets forwarded by the orchestrator (P0-T4).
    // Key derivation mirrors the sender in dispatch/mesh.rs: BLAKE3(VoxMeshJwtHmacSecret).
    let mut secret_bag: Option<crate::a2a::secret_bag::SecretBag> = None;
    if let Some(jwe) = msg.jwe_payload.as_deref() {
        let mesh_secret = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshJwtHmacSecret);
        if let Some(mesh_val) = mesh_secret.expose() {
            let derived = blake3::hash(mesh_val.as_bytes());
            match super::jwe::decrypt_jwe_compact(jwe, derived.as_bytes()) {
                Ok(plain) => match serde_json::from_slice::<serde_json::Value>(&plain) {
                    Ok(value) => match crate::a2a::secret_bag::SecretBag::from_decrypted(value) {
                        Ok(bag) => {
                            tracing::info!(
                                task_id = envelope.task_id,
                                message_id = msg.id,
                                secret_count = bag.len(),
                                "populi remote worker: SecretBag ready for declared injection",
                            );
                            secret_bag = Some(bag);
                        }
                        Err(e) => tracing::warn!(
                            task_id = envelope.task_id,
                            message_id = msg.id,
                            error = %e,
                            "populi remote worker: SecretBag construction failed",
                        ),
                    },
                    Err(e) => tracing::warn!(
                        task_id = envelope.task_id,
                        message_id = msg.id,
                        error = %e,
                        "populi remote worker: secret payload not JSON object",
                    ),
                },
                Err(e) => tracing::warn!(
                    task_id = envelope.task_id,
                    message_id = msg.id,
                    error = %e,
                    "populi remote worker: JWE decrypt failed; proceeding without forwarded secrets"
                ),
            }
        }
    }
    let _ = secret_bag; // threaded to skill runtime in Phase 5 (sandbox tiering)

    let payload_context = parse_remote_payload_context(&envelope.payload);
    let envelope_session_id = envelope
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);
    let envelope_context_json = envelope
        .context_envelope_json
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string);
    let effective_session_id = payload_context.session_id.or(envelope_session_id);
    let effective_context_json = payload_context
        .context_envelope_json
        .or(envelope_context_json);
    let effective_thread_id = payload_context.thread_id.or_else(|| {
        envelope
            .thread_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
    });
    let effective_harness_json = payload_context.harness_spec_json.or_else(|| {
        envelope
            .harness_spec_json
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string)
    });
    if let (Some(session_id), Some(context_envelope_json)) = (
        effective_session_id.as_deref(),
        effective_context_json.as_deref(),
    ) {
        match serde_json::from_str::<crate::ContextEnvelope>(context_envelope_json) {
            Ok(_) => {
                let key = crate::socrates::session_context_envelope_key(session_id);
                crate::sync_lock::rw_write(&*orchestrator.context_store).set(
                    crate::types::AgentId(0),
                    key,
                    context_envelope_json,
                    3600,
                );
                let seeded = orchestrator.attach_session_retrieval_envelope_if_present(
                    crate::types::TaskId(envelope.task_id),
                    &Some(session_id.to_string()),
                );
                tracing::debug!(
                    message_id = msg.id,
                    task_id = envelope.task_id,
                    session_id,
                    thread_id = effective_thread_id.as_deref(),
                    seeded,
                    "populi remote worker: seeded context store and attempted Socrates attach"
                );
            }
            Err(err) => {
                tracing::debug!(
                    message_id = msg.id,
                    error = %err,
                    payload_len = envelope.payload.len(),
                    "populi remote worker: context_envelope_json parse failed"
                );
            }
        }
    }
    if let Some(harness_spec_json) = effective_harness_json.as_deref() {
        match serde_json::from_str::<crate::AgentHarnessSpec>(harness_spec_json) {
            Ok(harness) => {
                let expectations = crate::HarnessIngestExpectations {
                    repository_id: envelope.repository_id.as_str(),
                    session_id: effective_session_id.as_deref(),
                    thread_id: effective_thread_id.as_deref(),
                };
                if let Err(errs) = crate::validate_agent_harness_ingest(&harness, expectations) {
                    tracing::warn!(
                        message_id = msg.id,
                        task_id = envelope.task_id,
                        errors = %errs.join("; "),
                        "populi remote worker: harness_spec_json failed validation"
                    );
                } else {
                    tracing::debug!(
                        message_id = msg.id,
                        task_id = envelope.task_id,
                        harness_id = %harness.harness_id,
                        thread_id = effective_thread_id.as_deref(),
                        "populi remote worker: accepted portable harness contract"
                    );
                }
            }
            Err(err) => tracing::warn!(
                message_id = msg.id,
                task_id = envelope.task_id,
                error = %err,
                "populi remote worker: harness_spec_json parse failed"
            ),
        }
    }
    // Lease-gated submit: orchestrator holds `task:{task_id}` and passes `exec_lease_id` in the envelope.
    // The worker must not grant a second lease (would conflict on scope) or renew/release as the wrong claimer.
    let orchestrator_holds_lease = !exec_lease_id.is_empty();

    let mut worker_owned_lease_id: Option<String> = None;
    if orchestrator_holds_lease {
        // No worker-side exec lease RPCs; orchestrator renews/releases.
    } else {
        // Legacy / demo: worker acquires a lease keyed like the orchestrator (`task:{task_id}`), not idempotency.
        let scope_key = format!("task:{}", envelope.task_id);
        let lease = match client
            .exec_lease_grant(&vox_populi::transport::RemoteExecLeaseGrantRequest {
                claimer_node_id: node_id.to_string(),
                scope_key,
            })
            .await
        {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!(
                    message_id = msg.id,
                    error = %e,
                    "populi remote worker: lease grant failed; leave inbox row for retry"
                );
                return;
            }
        };
        worker_owned_lease_id = Some(lease.lease_id.clone());
        let _ = client
            .exec_lease_renew(&vox_populi::transport::RemoteExecLeaseRenewRequest {
                lease_id: lease.lease_id,
                claimer_node_id: node_id.to_string(),
            })
            .await;
    }

    let result_payload = RemoteTaskResult {
        idempotency_key: envelope.idempotency_key.clone(),
        task_id: Some(envelope.task_id),
        success: true,
        result: Some(format!(
            "remote worker accepted payload ({} bytes)",
            envelope.payload.len()
        )),
        error: None,
    };
    let result_json = match serde_json::to_string(&result_payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::debug!(
                message_id = msg.id,
                error = %e,
                "populi remote worker: result serialization failed"
            );
            if let Some(ref lid) = worker_owned_lease_id {
                let _ = client
                    .exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
                        lease_id: lid.clone(),
                        claimer_node_id: node_id.to_string(),
                    })
                    .await;
            }
            return;
        }
    };

    let deliver_res = client
        .relay_a2a(&vox_populi::transport::A2ADeliverRequest {
            sender_agent_id: receiver_agent.to_string(),
            receiver_agent_id: sender_agent.to_string(),
            message_type: REMOTE_TASK_RESULT_TYPE.to_string(),
            payload: result_json,
            idempotency_key: Some(format!(
                "remote-result-{}-{}",
                envelope.task_id, envelope.idempotency_key
            )),
            privacy_class: envelope.privacy_class.clone(),
            payload_blake3_hex: None,
            worker_ed25519_sig_b64: None,
            jwe_payload: None,
            task_kind: None,
            model_id: None,
            traceparent: msg.traceparent.clone(),
            priority: 128,
        })
        .await;
    if deliver_res.is_err() {
        tracing::debug!(
            message_id = msg.id,
            "vox.mesh.trace_id" = trace_id,
            "populi remote worker: result delivery failed; leave source row for retry"
        );
        if let Some(ref lid) = worker_owned_lease_id {
            let _ = client
                .exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
                    lease_id: lid.clone(),
                    claimer_node_id: node_id.to_string(),
                })
                .await;
        }
        return;
    }

    tracing::info!(
        task_id = envelope.task_id,
        message_id = msg.id,
        "vox.mesh.trace_id" = trace_id,
        "populi remote worker: envelope processed and acked"
    );
    let _ = client
        .relay_a2a_ack(&receiver_agent.to_string(), msg.id)
        .await;
    if let Some(ref lid) = worker_owned_lease_id {
        let _ = client
            .exec_lease_release(&vox_populi::transport::RemoteExecLeaseReleaseRequest {
                lease_id: lid.clone(),
                claimer_node_id: node_id.to_string(),
            })
            .await;
    }
    }
    .instrument(span)
    .await;
}

async fn run_remote_worker_tick(
    orchestrator: &crate::orchestrator::Orchestrator,
    client: &vox_populi::http_client::PopuliHttpClient,
    receiver_agent: u64,
    sender_agent: u64,
) {
    let Ok(inbox) = client.relay_a2a_inbox(&receiver_agent.to_string()).await else {
        tracing::debug!(
            receiver_agent = receiver_agent,
            "populi remote worker: inbox HTTP failed"
        );
        return;
    };

    let node_id = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
        .expose()
        .map(str::to_string)
        .unwrap_or_else(|| "vox-orch-worker".to_string());
    for msg in inbox.messages {
        if msg.message_type != REMOTE_TASK_ENVELOPE_TYPE {
            continue;
        }
        let trace_id = msg
            .traceparent
            .as_deref()
            .and_then(|tp| tp.split('-').nth(1))
            .filter(|s| s.len() == 32)
            .map(str::to_string);
        let span = tracing::info_span!(
            "populi.remote_worker.process_envelope",
            message_id = msg.id,
            "vox.mesh.trace_id" = trace_id.as_deref().unwrap_or(""),
        );
        process_one_envelope(
            orchestrator,
            client,
            sender_agent,
            receiver_agent,
            msg,
            &node_id,
        )
        .instrument(span)
        .await;
    }
}

/// Spawn a periodic worker poll loop that consumes `remote_task_envelope` rows.
pub fn spawn_populi_remote_worker_poller(
    orchestrator: Arc<crate::orchestrator::Orchestrator>,
    join_slot: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
) {
    let mut guard = join_slot.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = guard.take() {
        h.abort();
    }
    let orch = orchestrator.clone();
    *guard = Some(tokio::spawn(async move {
        loop {
            let (interval_secs, run, url, timeout_ms, receiver_agent, sender_agent) = {
                let cfg = crate::sync_lock::rw_read(&*orch.config).clone();
                if !cfg.populi_remote_execute_experimental
                    || cfg.populi_remote_worker_poll_interval_secs == 0
                {
                    (5_u64, false, String::new(), 500_u64, 0_u64, 0_u64)
                } else {
                    let maybe_url = cfg
                        .populi_control_url
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    if let Some(url) = maybe_url {
                        let receiver_agent = cfg
                            .populi_remote_execute_receiver_agent
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(2_u64);
                        let sender_agent = cfg
                            .populi_remote_execute_sender_agent
                            .as_deref()
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(1_u64);
                        (
                            cfg.populi_remote_worker_poll_interval_secs.max(1),
                            true,
                            url,
                            cfg.populi_http_timeout_ms.max(500),
                            receiver_agent,
                            sender_agent,
                        )
                    } else {
                        (5_u64, false, String::new(), 500_u64, 0_u64, 0_u64)
                    }
                }
            };

            if run {
                let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
                    &url,
                    std::time::Duration::from_millis(timeout_ms),
                )
                .with_env_token();
                run_remote_worker_tick(&orch, &client, receiver_agent, sender_agent).await;
            }
            tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
        }
    }));
}

/// One-shot remote worker tick using current orchestrator config.
pub async fn populi_remote_worker_tick_once(orchestrator: &crate::orchestrator::Orchestrator) {
    let cfg = crate::sync_lock::rw_read(&*orchestrator.config).clone();
    if !cfg.populi_remote_execute_experimental || cfg.populi_remote_worker_poll_interval_secs == 0 {
        return;
    }
    let Some(url) = cfg
        .populi_control_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return;
    };
    let receiver_agent = cfg
        .populi_remote_execute_receiver_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(2_u64);
    let sender_agent = cfg
        .populi_remote_execute_sender_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1_u64);
    let timeout_ms = cfg.populi_http_timeout_ms.max(500);
    let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
        url,
        std::time::Duration::from_millis(timeout_ms),
    )
    .with_env_token();
    run_remote_worker_tick(orchestrator, &client, receiver_agent, sender_agent).await;
}

#[cfg(test)]
mod tests {
    use super::parse_remote_payload_context;

    #[test]
    fn parse_remote_payload_context_extracts_session_and_context() {
        let payload = serde_json::json!({
            "task_description": "x",
            "session_id": "  sid-123 ",
            "thread_id": " thread-123 ",
            "context_envelope_json": "{\"schema_version\":1}",
            "harness_spec_json": "{\"schema_version\":1}"
        })
        .to_string();
        let parsed = parse_remote_payload_context(&payload);
        assert_eq!(parsed.session_id.as_deref(), Some("sid-123"));
        assert_eq!(parsed.thread_id.as_deref(), Some("thread-123"));
        assert_eq!(
            parsed.context_envelope_json.as_deref(),
            Some("{\"schema_version\":1}")
        );
        assert_eq!(
            parsed.harness_spec_json.as_deref(),
            Some("{\"schema_version\":1}")
        );
    }

    #[test]
    fn parse_remote_payload_context_handles_missing_fields() {
        let parsed = parse_remote_payload_context("{\"task_description\":\"x\"}");
        assert!(parsed.session_id.is_none());
        assert!(parsed.thread_id.is_none());
        assert!(parsed.context_envelope_json.is_none());
        assert!(parsed.harness_spec_json.is_none());
    }

    #[test]
    fn parse_remote_payload_context_serializes_object_form_context_envelope() {
        let payload = serde_json::json!({
            "session_id": "sid-obj",
            "context_envelope_json": {
                "schema_version": 1,
                "envelope_type": "retrieval_evidence"
            }
        })
        .to_string();
        let parsed = parse_remote_payload_context(&payload);
        assert_eq!(parsed.session_id.as_deref(), Some("sid-obj"));
        let context = parsed
            .context_envelope_json
            .as_deref()
            .expect("context json should be captured");
        let as_value: serde_json::Value = serde_json::from_str(context).expect("valid json");
        assert_eq!(as_value["schema_version"], 1);
        assert_eq!(as_value["envelope_type"], "retrieval_evidence");
    }
}
