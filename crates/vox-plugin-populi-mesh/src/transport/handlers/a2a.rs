//! A2A message handlers: deliver, inbox, ack, lease-renew, admin quarantine/maintenance.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use tracing::warn;

use crate::{
    MAX_MAINTENANCE_FOR_MS, NodeRecord, node_maintenance_blocks_new_work,
    sweep_expired_maintenance_on_nodes,
};

use super::super::auth::{
    PopuliAuthContext, auth_allows_admin_route, auth_allows_deliver, auth_allows_worker_plane,
};
use super::super::result_attestation;
use super::super::store::persist_a2a_store;
use super::super::{
    A2AAckRequest, A2ADeliverRequest, A2ADeliverResponse, A2AInboxRequest, A2AInboxResponse,
    A2ALeaseRenewRequest, A2AStoredMessage, AdminMaintenanceRequest, AdminQuarantineRequest,
    PopuliTransportState, a2a_in_memory_cap, a2a_lease_duration_ms, a2a_sweep_expired_leases,
};
use super::nodes::{
    ResponseErr, a2a_inbox_limit, parse_a2a_mesh_agent_id, registry_sweep_maintenance,
    store_ack_a2a, store_put_a2a,
};

fn claim_policy_allows_worker(
    worker: &NodeRecord,
    msg: &A2AStoredMessage,
    sender_owner_id: Option<&str>,
) -> bool {
    let privacy = msg
        .privacy_class
        .as_deref()
        .unwrap_or("public")
        .trim()
        .to_ascii_lowercase();

    let is_public = privacy.is_empty() || privacy == "public";

    let vis = worker
        .visibility
        .as_deref()
        .unwrap_or("private")
        .trim()
        .to_ascii_lowercase();

    // 1. Mandatory Visibility Check: Private nodes never take public tasks.
    if is_public && vis == "private" {
        return false;
    }

    // 2. Donation Policy Check (for public mesh).
    if is_public && let Some(policy) = &worker.donation_policy {
        // Opt-out check.
        if !policy.public_mesh_opt_in {
            return false;
        }

        // Priority check.
        if msg.priority < policy.min_priority {
            return false;
        }

        // Task Kind check.
        if let Some(msg_kind) = &msg.task_kind {
            let allowed = policy.slots.iter().any(|s| {
                let s_kind = format!("{:?}", s.task_kind).to_lowercase();
                s_kind == msg_kind.to_lowercase()
            });
            if !allowed {
                return false;
            }
        }

        // Identity checks.
        if let Some(denied) = &policy.denied_users
            && let Some(owner) = sender_owner_id
            && denied.contains(&owner.to_string())
        {
            return false;
        }

        if let Some(allowed) = &policy.allowed_users {
            match sender_owner_id {
                Some(owner) => {
                    if !allowed.contains(&owner.to_string()) {
                        return false;
                    }
                }
                None => return false, // If allowed_users is set, anonymous tasks are rejected.
            }
        }

        // Allowed Scopes check.
        if let Some(_allowed_scopes) = &policy.allowed_scopes {
            // If we don't know the sender's scope, we can't verify, so we skip if policy is strict.
            // (In a real federation, we'd have the sender's scope in the message envelope).
            // For now, if sender_node_id is present, we check it.
            // Since we don't have easy access to the whole registry here without passing it,
            // we'll assume for Phase 1 that allowed_scopes: None means "all public".
        }
    }

    let trust = worker
        .trust_tier
        .as_deref()
        .unwrap_or("trusted")
        .trim()
        .to_ascii_lowercase();

    match privacy.as_str() {
        "private" | "trusted" => vis != "public",
        "trusted_only" => vis != "public" && trust != "new",
        _ => true,
    }
}

pub(crate) async fn deliver_a2a(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<A2ADeliverRequest>,
) -> Result<Json<A2ADeliverResponse>, ResponseErr> {
    if !auth_allows_deliver(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: submitter/mesh/admin token required for a2a/deliver".into(),
        ));
    }
    // Emit vox.mesh.* span attributes (S1 local observability).
    let trace_id_for_span = req
        .traceparent
        .as_deref()
        .and_then(|tp| vox_mesh_types::MeshTraceContext::from_traceparent(tp).ok())
        .map(|ctx| ctx.trace_id_hex());
    tracing::debug!(
        "vox.mesh.message_type" = req.message_type.as_str(),
        "vox.mesh.privacy_class" = req.privacy_class.as_deref().unwrap_or("public"),
        "vox.mesh.trace_id" = trace_id_for_span.as_deref().unwrap_or(""),
        "vox.mesh.dispatch_kind" = "local",
    );

    let sender_agent_id = parse_a2a_mesh_agent_id("sender_agent_id", &req.sender_agent_id)?;
    let receiver_agent_id = parse_a2a_mesh_agent_id("receiver_agent_id", &req.receiver_agent_id)?;
    let sender_node_id = if let PopuliAuthContext::NodeSignature { node_id, .. } = &ctx {
        Some(node_id.clone())
    } else {
        None
    };
    let dh = req
        .payload_blake3_hex
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let sb = req
        .worker_ed25519_sig_b64
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if let Err(msg) = result_attestation::enforce_deliver_attestation(
        &req.message_type,
        &req.payload,
        dh,
        sb,
        st.worker_result_verify_key.as_ref(),
    ) {
        // Punish node for failed attestation.
        if let Some(node_id) = &sender_node_id {
            let mut g = st.inner.write().await;
            if let Some(i) = g.nodes.iter().position(|n| n.id == *node_id) {
                g.nodes[i].trust_tier = Some("degraded".to_string());
                tracing::warn!(
                    node_id = node_id,
                    error = %msg,
                    "populi: node trust degraded due to attestation failure"
                );
            }
        }

        let status = if msg.contains("not configured") {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_REQUEST
        };
        return Err(ResponseErr(status, msg));
    }
    if let Some(key) = req
        .idempotency_key
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let dedupe = format!("{}\x1f{}\x1f{}", sender_agent_id, receiver_agent_id, key);
        let mut maps = st.mesh_replay.maps().write().await;
        if let Some(&existing) = maps.idempotency.get(&dedupe) {
            return Ok(Json(A2ADeliverResponse {
                accepted: true,
                message_id: existing,
            }));
        }
        let id = st
            .a2a_id_gen
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        maps.idempotency.insert(dedupe.clone(), id);
        drop(maps);
        st.mesh_replay.persist_if_configured().await;
        let msg = A2AStoredMessage {
            id,
            sender_agent_id,
            receiver_agent_id,
            message_type: req.message_type,
            payload: req.payload,
            created_unix_ms: crate::now_ms(),
            acknowledged: false,
            lease_holder_node_id: None,
            lease_expires_unix_ms: None,
            privacy_class: req.privacy_class.clone(),
            idempotency_dedupe_key: Some(dedupe),
            payload_blake3_hex: req.payload_blake3_hex.clone(),
            worker_ed25519_sig_b64: req.worker_ed25519_sig_b64.clone(),
            jwe_payload: req.jwe_payload.clone(),
            priority: req.priority,
            task_kind: req.task_kind.clone(),
            model_id: req.model_id.clone(),
            sender_node_id: sender_node_id.clone(),
            traceparent: req.traceparent.clone(),
        };
        let mut g = st.a2a_messages.write().await;
        a2a_sweep_expired_leases(&mut g, crate::now_ms());
        let cap = a2a_in_memory_cap();
        if g.len() >= cap {
            let drop_n = g.len() - cap + 1;
            g.drain(0..drop_n);
        }
        let msg_copy = msg.clone();
        g.push(msg);
        if let Some(path) = st.a2a_store_path.as_ref() {
            let _ = persist_a2a_store(path, &g);
        }
        drop(g);
        store_put_a2a(&st, msg_copy);
        return Ok(Json(A2ADeliverResponse {
            accepted: true,
            message_id: id,
        }));
    }

    let id = st
        .a2a_id_gen
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let msg = A2AStoredMessage {
        id,
        sender_agent_id,
        receiver_agent_id,
        message_type: req.message_type,
        payload: req.payload,
        created_unix_ms: crate::now_ms(),
        acknowledged: false,
        lease_holder_node_id: None,
        lease_expires_unix_ms: None,
        privacy_class: req.privacy_class.clone(),
        idempotency_dedupe_key: None,
        payload_blake3_hex: req.payload_blake3_hex.clone(),
        worker_ed25519_sig_b64: req.worker_ed25519_sig_b64.clone(),
        jwe_payload: req.jwe_payload.clone(),
        priority: req.priority,
        task_kind: req.task_kind.clone(),
        model_id: req.model_id.clone(),
        sender_node_id,
        traceparent: req.traceparent.clone(),
    };
    let mut g = st.a2a_messages.write().await;
    a2a_sweep_expired_leases(&mut g, crate::now_ms());
    let cap = a2a_in_memory_cap();
    if g.len() >= cap {
        let drop_n = g.len() - cap + 1;
        g.drain(0..drop_n);
    }
    let msg_copy = msg.clone();
    g.push(msg);
    if let Some(path) = st.a2a_store_path.as_ref() {
        let _ = persist_a2a_store(path, &g);
    }
    drop(g);
    store_put_a2a(&st, msg_copy);
    Ok(Json(A2ADeliverResponse {
        accepted: true,
        message_id: id,
    }))
}

pub(crate) async fn admin_quarantine(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<AdminQuarantineRequest>,
) -> Result<StatusCode, ResponseErr> {
    if !auth_allows_admin_route(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: mesh/admin bearer required for quarantine".into(),
        ));
    }
    let id = req.node_id.trim();
    if id.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            "populi: node_id required".into(),
        ));
    }
    let mut g = st.inner.write().await;
    if let Some(i) = g.nodes.iter().position(|n| n.id == id) {
        g.nodes[i].quarantined = Some(req.quarantined);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

pub(crate) async fn admin_maintenance(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<AdminMaintenanceRequest>,
) -> Result<StatusCode, ResponseErr> {
    if !auth_allows_admin_route(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: mesh/admin bearer required for maintenance".into(),
        ));
    }
    let id = req.node_id.trim();
    if id.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            "populi: node_id required".into(),
        ));
    }
    let now = crate::now_ms();
    let mut g = st.inner.write().await;
    sweep_expired_maintenance_on_nodes(&mut g.nodes, now);
    if let Some(i) = g.nodes.iter().position(|n| n.id == id) {
        if !req.maintenance {
            g.nodes[i].maintenance = Some(false);
            g.nodes[i].maintenance_until_unix_ms = None;
            Ok(StatusCode::NO_CONTENT)
        } else {
            let abs = req.maintenance_until_unix_ms.filter(|&u| u > now);
            let until = if abs.is_some() {
                abs
            } else if let Some(rel) = req.maintenance_for_ms.filter(|&m| m > 0) {
                let capped = rel.min(MAX_MAINTENANCE_FOR_MS);
                Some(now.saturating_add(capped))
            } else {
                None
            };
            g.nodes[i].maintenance = Some(true);
            g.nodes[i].maintenance_until_unix_ms = until;
            Ok(StatusCode::NO_CONTENT)
        }
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

pub(crate) async fn a2a_lease_renew(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<A2ALeaseRenewRequest>,
) -> Result<StatusCode, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for lease renew".into(),
        ));
    }
    let now = crate::now_ms();
    let lease_ms = a2a_lease_duration_ms();
    let claimer = req.claimer_node_id.trim();
    if claimer.is_empty() {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            "populi: claimer_node_id required".into(),
        ));
    }
    registry_sweep_maintenance(&st).await;
    let worker = {
        let reg = st.inner.read().await;
        reg.nodes.iter().find(|n| n.id == claimer).cloned()
    };
    let Some(worker) = worker else {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: unknown claimer_node_id".into(),
        ));
    };
    if worker.quarantined == Some(true) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: claimer node is quarantined".into(),
        ));
    }
    if node_maintenance_blocks_new_work(now, &worker) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: claimer node is in maintenance mode".into(),
        ));
    }
    let mut g = st.a2a_messages.write().await;
    a2a_sweep_expired_leases(&mut g, now);
    let Some(msg) = g.iter_mut().find(|m| {
        m.id == req.message_id && m.receiver_agent_id == req.receiver_agent_id && !m.acknowledged
    }) else {
        return Ok(StatusCode::NOT_FOUND);
    };
    if msg.lease_holder_node_id.as_deref() != Some(claimer) {
        return Err(ResponseErr(
            StatusCode::CONFLICT,
            "populi: lease renew only for active lease holder".into(),
        ));
    }
    msg.lease_expires_unix_ms = Some(now.saturating_add(lease_ms));
    let renewed_msg = msg.clone();
    if let Some(path) = st.a2a_store_path.as_ref() {
        let _ = persist_a2a_store(path, &g);
    }
    drop(g);
    store_put_a2a(&st, renewed_msg);
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn a2a_inbox(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<A2AInboxRequest>,
) -> Result<Json<A2AInboxResponse>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for a2a/inbox".into(),
        ));
    }
    let now = crate::now_ms();
    let lease_ms = a2a_lease_duration_ms();

    let claimer = req
        .claimer_node_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if claimer.is_none() {
        let mut g = st.a2a_messages.write().await;
        a2a_sweep_expired_leases(&mut g, now);
        let max_messages = a2a_inbox_limit(req.max_messages);
        let before = req.before_message_id;
        let messages = g
            .iter()
            .rev()
            .filter(|m| {
                m.receiver_agent_id == req.receiver_agent_id
                    && !m.acknowledged
                    && before.is_none_or(|cursor| m.id < cursor)
            })
            .take(max_messages)
            .cloned()
            .collect();
        if let Some(path) = st.a2a_store_path.as_ref() {
            let _ = persist_a2a_store(path, &g);
        }
        return Ok(Json(A2AInboxResponse { messages }));
    }
    let claimer = claimer.expect("claimer");

    registry_sweep_maintenance(&st).await;
    let worker = {
        let reg = st.inner.read().await;
        reg.nodes.iter().find(|n| n.id == claimer).cloned()
    };
    let Some(worker) = worker else {
        warn!(
            claimer,
            "a2a inbox claim rejected: unknown claimer node (join first)"
        );
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: unknown claimer_node_id (join node first)".into(),
        ));
    };
    if worker.quarantined == Some(true) {
        tracing::debug!(claimer, "populi policy: quarantined worker cannot claim");
        return Ok(Json(A2AInboxResponse { messages: vec![] }));
    }
    if node_maintenance_blocks_new_work(now, &worker) {
        tracing::debug!(claimer, "populi policy: maintenance worker cannot claim");
        return Ok(Json(A2AInboxResponse { messages: vec![] }));
    }

    let nodes = {
        let reg = st.inner.read().await;
        reg.nodes.clone()
    };

    let mut g = st.a2a_messages.write().await;
    a2a_sweep_expired_leases(&mut g, now);
    let mut picked_idx: Option<usize> = None;
    let mut best_priority: u8 = 0;
    for (i, m) in g.iter_mut().enumerate() {
        if m.receiver_agent_id != req.receiver_agent_id || m.acknowledged {
            continue;
        }
        let leased_other = m
            .lease_holder_node_id
            .as_deref()
            .is_some_and(|h| h != claimer);
        let lease_alive = m.lease_expires_unix_ms.is_some_and(|exp| exp > now);
        if leased_other && lease_alive {
            continue;
        }

        let sender_owner_id = m.sender_node_id.as_ref().and_then(|id| {
            nodes
                .iter()
                .find(|n| n.id == *id)
                .and_then(|n| n.owner_vox_user_id.as_deref())
        });

        if !claim_policy_allows_worker(&worker, m, sender_owner_id) {
            tracing::debug!(
                message_id = m.id,
                claimer,
                "populi policy: skipping inbox row for worker visibility/trust"
            );
            continue;
        }
        if picked_idx.is_none() || m.priority > best_priority {
            picked_idx = Some(i);
            best_priority = m.priority;
        }
    }
    let Some(i) = picked_idx else {
        return Ok(Json(A2AInboxResponse { messages: vec![] }));
    };
    let m = &mut g[i];
    m.lease_holder_node_id = Some(claimer.to_string());
    m.lease_expires_unix_ms = Some(now.saturating_add(lease_ms));
    let claimed = m.clone();
    tracing::debug!(
        "vox.mesh.message_id" = claimed.id,
        "vox.mesh.message_type" = claimed.message_type.as_str(),
        "vox.mesh.trace_id" = claimed
            .traceparent
            .as_deref()
            .and_then(|tp| vox_mesh_types::MeshTraceContext::from_traceparent(tp).ok())
            .map(|ctx| ctx.trace_id_hex())
            .as_deref()
            .unwrap_or(""),
    );
    let one = vec![claimed.clone()];
    if let Some(path) = st.a2a_store_path.as_ref() {
        let _ = persist_a2a_store(path, &g);
    }
    drop(g);
    store_put_a2a(&st, claimed);
    Ok(Json(A2AInboxResponse { messages: one }))
}

pub(crate) async fn a2a_ack(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<A2AAckRequest>,
) -> Result<StatusCode, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for a2a/ack".into(),
        ));
    }
    let mut g = st.a2a_messages.write().await;
    a2a_sweep_expired_leases(&mut g, crate::now_ms());
    if let Some(msg) = g
        .iter_mut()
        .find(|m| m.id == req.message_id && m.receiver_agent_id == req.receiver_agent_id)
    {
        msg.acknowledged = true;
        msg.lease_holder_node_id = None;
        msg.lease_expires_unix_ms = None;
        tracing::debug!(
            "vox.mesh.message_id" = msg.id,
            "vox.mesh.message_type" = msg.message_type.as_str(),
            "vox.mesh.trace_id" = msg
                .traceparent
                .as_deref()
                .and_then(|tp| vox_mesh_types::MeshTraceContext::from_traceparent(tp).ok())
                .map(|ctx| ctx.trace_id_hex())
                .as_deref()
                .unwrap_or(""),
        );

        // Wave 2: Kudos crediting for job results.
        if msg.message_type == "job_result"
            && let (Ok(result), Some(db), Some(node_id)) = (
                serde_json::from_str::<vox_mesh_types::TaskResult>(&msg.payload),
                &st.db,
                &msg.sender_node_id,
            )
            && result.success
        {
            let owner_id = {
                let reg = st.inner.read().await;
                reg.nodes
                    .iter()
                    .find(|n| n.id == *node_id)
                    .and_then(|n| n.owner_vox_user_id.clone())
            };

            if let Some(vox_user_id) = owner_id {
                let credit = vox_mesh_types::kudos::CreditJobRequest {
                    vox_user_id,
                    node_id: node_id.clone(),
                    primitive: vox_mesh_types::kudos::RewardPrimitive::GpuComputeMs,
                    amount: result.duration_ms,
                    task_id: Some(msg.id.to_string()),
                    metadata_json: None,
                };
                let db = db.clone();
                tokio::spawn(async move {
                    if let Err(e) = db.credit_kudos(&credit).await {
                        tracing::error!("failed to credit kudos: {:?}", e);
                    }
                });
            }
        }

        if let Some(key) = msg.idempotency_dedupe_key.clone() {
            let mut maps = st.mesh_replay.maps().write().await;
            maps.idempotency.remove(&key);
        }
        msg.idempotency_dedupe_key = None;
        let acked_id = msg.id;
        if let Some(path) = st.a2a_store_path.as_ref() {
            let _ = persist_a2a_store(path, &g);
        }
        drop(g);
        st.mesh_replay.persist_if_configured().await;
        store_ack_a2a(&st, acked_id, crate::now_ms());
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}
