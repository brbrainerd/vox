//! Best-effort Populi / mens HTTP steps (feature `mens`).

#[cfg(feature = "mens")]
use anyhow::anyhow;
#[cfg(feature = "mens")]
use serde_json::{Value, json};

#[cfg(feature = "mens")]
use super::types::{PopuliActivity, PopuliHttpOp};

/// Best-effort mens registration + optional control-plane HTTP (env-derived base URL only).
#[cfg(feature = "mens")]
pub async fn execute_populi_step(activity: &PopuliActivity) -> anyhow::Result<Value> {
    let _ = vox_populi::publish_local_registry_best_effort();
    let vox = vox_populi::resolve_vox_toml_best_effort();
    let env = vox_populi::populi_env_resolved(vox.as_deref());
    let timeout = activity
        .timeout_ms
        .map_or(vox_config::timeouts::HTTP_REQUEST, |ms| {
            std::time::Duration::from_millis(ms.max(250))
        });
    if let Some(base) = env.control_addr.clone() {
        let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
            normalize_control_base(&base),
            timeout,
        )
        .with_env_token();
        let id = env
            .node_id
            .clone()
            .unwrap_or_else(|| format!("wf-{}", activity.name.replace(' ', "_")));
        let node = vox_populi::node_record_for_current_process(id, Some(base.clone()));
        let mesh_op = populi_op_json(activity.populi_op);
        match activity.populi_op {
            PopuliHttpOp::Noop => Ok(json!({
                "event": "MeshActivity",
                "activity": activity.name,
                "activity_id": activity.activity_id,
                "mesh_op": mesh_op,
                "control": "noop",
            })),
            PopuliHttpOp::Join => match client.join(&node).await {
                Ok(n) => Ok(json!({
                    "event": "MeshActivity",
                    "activity": activity.name,
                    "activity_id": activity.activity_id,
                    "mesh_op": mesh_op,
                    "control": "join_ok",
                    "node_id": n.id,
                })),
                Err(e) => Err(anyhow!(
                    "mesh join failed for activity `{}`: {}",
                    activity.name,
                    e
                )),
            },
            PopuliHttpOp::Snapshot => match client.list_nodes().await {
                Ok(f) => Ok(json!({
                    "event": "MeshActivity",
                    "activity": activity.name,
                    "activity_id": activity.activity_id,
                    "mesh_op": mesh_op,
                    "control": "snapshot_ok",
                    "node_count": f.nodes.len(),
                    "schema_version": f.schema_version,
                })),
                Err(e) => Err(anyhow!(
                    "mesh snapshot failed for activity `{}`: {}",
                    activity.name,
                    e
                )),
            },
            PopuliHttpOp::Heartbeat => match client.heartbeat(&node).await {
                Ok(n) => Ok(json!({
                    "event": "MeshActivity",
                    "activity": activity.name,
                    "activity_id": activity.activity_id,
                    "mesh_op": mesh_op,
                    "control": "heartbeat_ok",
                    "node_id": n.id,
                })),
                Err(e) => Err(anyhow!(
                    "mesh heartbeat failed for activity `{}`: {}",
                    activity.name,
                    e
                )),
            },
            PopuliHttpOp::Dispatch => {
                use base64::Engine as _;
                // For an interpreted workflow, the dispatched source is a synthesized runner for the activity.
                let shim = format!(
                    "workflow_durable_shim::execute_activity(\"{}\");\n",
                    activity.name
                );
                let b64_source = base64::engine::general_purpose::STANDARD.encode(shim);
                let req = vox_populi::transport::DispatchRequest {
                    source: b64_source,
                    node_id: None, // Can be extended to pin to a specific agent id via properties
                    timeout_secs: activity.timeout_ms.map(|t| (t / 1000).max(1)).unwrap_or(30),
                    is_bundle: false,
                    source_blake3_hex: None,
                    required_labels: activity.required_labels.clone(),
                    is_detached: activity.is_detached,
                    priority: 128,
                    task_kind: Some("vox_script".to_string()),
                    model_id: None,
                    min_vram_mb: None,
                };
                match client.dispatch(&req).await {
                    Ok(res) => Ok(json!({
                        "event": "MeshActivity",
                        "activity": activity.name,
                        "activity_id": activity.activity_id,
                        "mesh_op": mesh_op,
                        "control": "dispatch_ok",
                        "dispatch_id": res.node_id, // If detached, this should hold the Job ID or dispatch_id
                        "success": res.success,
                        "result_output": res.output,
                        "exit_code": res.exit_code,
                    })),
                    Err(e) => Err(anyhow!(
                        "mesh dispatch failed for activity `{}`: {}",
                        activity.name,
                        e
                    )),
                }
            }
            PopuliHttpOp::Wait => {
                // The activity name is conventionally the tracking ID for the Wait operation
                // Activity ID serves as uniqueness
                let dispatch_id = &activity.name;
                match client.dispatch_result_poll(dispatch_id).await {
                    Ok(res) => Ok(json!({
                        "event": "MeshActivity",
                        "activity": activity.name,
                        "activity_id": activity.activity_id,
                        "mesh_op": mesh_op,
                        "control": "wait_ok",
                        "success": res.success,
                        "result_output": res.output,
                        "exit_code": res.exit_code,
                    })),
                    Err(e) => Err(anyhow!(
                        "mesh wait polling failed for activity `{}`: {}",
                        activity.name,
                        e
                    )),
                }
            }
        }
    } else {
        Ok(json!({
            "event": "MeshActivity",
            "activity": activity.name,
            "activity_id": activity.activity_id,
            "mesh_op": populi_op_json(activity.populi_op),
            "control": "local_registry_only",
        }))
    }
}

#[cfg(feature = "mens")]
fn populi_op_json(op: PopuliHttpOp) -> &'static str {
    match op {
        PopuliHttpOp::Heartbeat => "heartbeat",
        PopuliHttpOp::Noop => "noop",
        PopuliHttpOp::Join => "join",
        PopuliHttpOp::Snapshot => "snapshot",
        PopuliHttpOp::Dispatch => "dispatch",
        PopuliHttpOp::Wait => "wait",
    }
}

#[cfg(feature = "mens")]
fn normalize_control_base(addr: &str) -> String {
    let a = addr.trim();
    if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        format!("http://{a}")
    }
}
