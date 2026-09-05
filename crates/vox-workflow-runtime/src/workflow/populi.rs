//! Populi / mens activity steps (feature `mens`).
//!
//! [`PopuliHttpOp`] is a Vox `activity` **language surface**, so it is ported
//! onto the iroh mesh rather than retired (plan Task 0.3 Step 5 / Task 3.4).
//! It now spans two planes, and which plane an op lands on is decided by what
//! the mesh can honestly do:
//!
//! - **`Noop`, `Snapshot`, `Join`, `Heartbeat` run on the mesh.** No HTTP, no
//!   control plane. `Snapshot` is `vox_mesh_transport::directory()` — peers
//!   that answered a `Probe` just now, not a list somebody asserted.
//! - **`Dispatch` and `Wait` cannot be served by the mesh.** The mesh probes
//!   and does not execute: `ProbeOnlyExecutor` refuses `Run` because no sandbox
//!   exists, and wiring an executor to `Run` anyway is the failure this program
//!   exists to prevent. They keep the legacy HTTP plane (deleted in Phase 6),
//!   and error clearly when it is not configured.
//!
//! `Join` and `Heartbeat` have no mesh equivalent to *perform*: on iroh there
//! is no control plane to register with — membership **is** pairing
//! (`vox mesh join <ticket>`) and reachability is **demonstrated** by the
//! probe, never asserted by a POST. So they report that state plainly instead
//! of emitting a `join_ok` / `heartbeat_ok` for something that did not happen.
//!
//! The `event` / `activity` / `activity_id` / `mesh_op` keys are an observable
//! contract of the language surface and are unchanged.

#[cfg(feature = "mens")]
use anyhow::anyhow;
#[cfg(feature = "mens")]
use serde_json::{Value, json};

#[cfg(feature = "mens")]
use super::types::{PopuliActivity, PopuliHttpOp};

/// Execute one mens activity step: mesh plane, or legacy HTTP for the two ops
/// the mesh cannot serve.
#[cfg(feature = "mens")]
pub async fn execute_populi_step(activity: &PopuliActivity) -> anyhow::Result<Value> {
    let _ = vox_populi::publish_local_registry_best_effort();
    match activity.populi_op {
        PopuliHttpOp::Noop
        | PopuliHttpOp::Join
        | PopuliHttpOp::Snapshot
        | PopuliHttpOp::Heartbeat => Ok(execute_mesh_step(activity).await),
        PopuliHttpOp::Dispatch | PopuliHttpOp::Wait => execute_http_step(activity).await,
    }
}

/// The mesh plane. Never fails: an absent or unreadable mesh means "no peers",
/// which is a fact about the network, not an error in the workflow.
#[cfg(feature = "mens")]
async fn execute_mesh_step(activity: &PopuliActivity) -> Value {
    let op = activity.populi_op;
    let control = mesh_control_label(op);
    match op {
        PopuliHttpOp::Noop => mesh_envelope(activity, control, json!({})),
        PopuliHttpOp::Join => mesh_envelope(
            activity,
            control,
            json!({
                // Read from the key, not from a bound endpoint: answering "who
                // am I on the mesh" must not open sockets.
                "endpoint_id": vox_mesh_transport::load_or_create(&vox_dir().join("mesh.key"))
                    .ok()
                    .map(|sk| sk.public().to_string()),
                "trusted_peers": mesh_trust().rows().len(),
                "detail": "iroh has no control plane to register with: membership is pairing. \
                           Run `vox mesh join <ticket>` to pair with a peer.",
            }),
        ),
        // Both are "who is actually out there right now". Snapshot asks it as a
        // directory listing; Heartbeat asks it as a liveness check, which on a
        // probe-based mesh is the same round-trip.
        PopuliHttpOp::Snapshot | PopuliHttpOp::Heartbeat => {
            let peers = probed_peers().await;
            mesh_envelope(
                activity,
                control,
                json!({
                    "node_count": peers.len(),
                    "peers": peers,
                    "detail": "peers that answered a Probe just now; a trusted peer that is \
                               switched off is absent rather than listed",
                }),
            )
        }
        // Routed to the HTTP plane by `execute_populi_step`.
        PopuliHttpOp::Dispatch | PopuliHttpOp::Wait => {
            mesh_envelope(activity, "unreachable", json!({}))
        }
    }
}

/// The legacy HTTP plane, kept only for `Dispatch` and `Wait`. Phase 6 deletes it.
#[cfg(feature = "mens")]
async fn execute_http_step(activity: &PopuliActivity) -> anyhow::Result<Value> {
    let vox = vox_populi::resolve_vox_toml_best_effort();
    let env = vox_populi::populi_env_resolved(vox.as_deref());
    let timeout = activity
        .timeout_ms
        .map_or(vox_config::timeouts::HTTP_REQUEST, |ms| {
            std::time::Duration::from_millis(ms.max(250))
        });
    let Some(base) = env.control_addr.clone() else {
        return Err(no_execution_plane(activity, activity.populi_op));
    };
    let client = vox_populi::http_client::PopuliHttpClient::new_with_timeout(
        normalize_control_base(&base),
        timeout,
    )
    .with_env_token();
    let mesh_op = populi_op_json(activity.populi_op);
    match activity.populi_op {
        PopuliHttpOp::Noop
        | PopuliHttpOp::Join
        | PopuliHttpOp::Snapshot
        | PopuliHttpOp::Heartbeat => Ok(execute_mesh_step(activity).await),
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
}

/// `Dispatch` / `Wait` with no HTTP control plane configured.
///
/// The old code returned a `local_registry_only` **success** envelope here,
/// which told a workflow that a dispatch had happened when nothing ran at all.
#[cfg(feature = "mens")]
fn no_execution_plane(activity: &PopuliActivity, op: PopuliHttpOp) -> anyhow::Error {
    anyhow!(
        "mesh {} is unavailable for activity `{}`: the iroh mesh probes peers but \
         cannot execute work on them (no sandbox exists, so the mesh executor refuses \
         Run), and no HTTP control plane is configured. Set VOX_MESH_CONTROL_ADDR (or \
         `[populi] control_addr` in Vox.toml) to use the legacy dispatch plane, or \
         replace this activity with a local one.",
        populi_op_json(op),
        activity.name,
    )
}

/// Merge `extra` into the frozen `MeshActivity` envelope.
///
/// One place so the four contract keys cannot drift per op.
#[cfg(feature = "mens")]
fn mesh_envelope(activity: &PopuliActivity, control: &str, extra: Value) -> Value {
    let mut v = json!({
        "event": "MeshActivity",
        "activity": activity.name,
        "activity_id": activity.activity_id,
        "mesh_op": populi_op_json(activity.populi_op),
        "control": control,
    });
    if let (Some(obj), Value::Object(extra)) = (v.as_object_mut(), extra) {
        obj.extend(extra);
    }
    v
}

/// What the mesh plane honestly did, per op.
#[cfg(feature = "mens")]
fn mesh_control_label(op: PopuliHttpOp) -> &'static str {
    match op {
        PopuliHttpOp::Noop => "noop",
        // Not `join_ok`: nothing was joined. Pairing is out of band.
        PopuliHttpOp::Join => "pairing_is_out_of_band",
        PopuliHttpOp::Snapshot => "snapshot_ok",
        // Not `heartbeat_ok`: nobody was told we are alive. We asked instead.
        PopuliHttpOp::Heartbeat => "reachability_probed",
        PopuliHttpOp::Dispatch => "dispatch",
        PopuliHttpOp::Wait => "wait",
    }
}

/// `~/.vox` — must match where `vox mesh join` writes, or pairing and workflow
/// activities disagree about which peers exist.
#[cfg(feature = "mens")]
fn vox_dir() -> std::path::PathBuf {
    vox_config::paths::dot_vox_user_dir()
}

#[cfg(feature = "mens")]
fn mesh_trust() -> vox_mesh_transport::MeshTrust {
    vox_mesh_transport::MeshTrust::at(&vox_dir().join("mesh_trust.json"))
}

/// Bound once per process.
///
/// Binding an iroh endpoint opens sockets and starts background tasks, which is
/// far too heavy to repeat per activity step.
///
// vox:defactored-from vox-orchestrator 2026-09-05 — the same bind-once wrapper
// as `vox-orchestrator/src/models/mesh_directory.rs`, duplicated (25 lines)
// rather than taking a vox-workflow-runtime -> vox-orchestrator crate edge.
#[cfg(feature = "mens")]
static MESH_ENDPOINT: tokio::sync::OnceCell<Option<iroh::Endpoint>> =
    tokio::sync::OnceCell::const_new();

#[cfg(feature = "mens")]
async fn mesh_endpoint() -> Option<&'static iroh::Endpoint> {
    MESH_ENDPOINT
        .get_or_init(|| async {
            let sk = match vox_mesh_transport::load_or_create(&vox_dir().join("mesh.key")) {
                Ok(sk) => sk,
                Err(e) => {
                    tracing::warn!(target: "vox.workflow.mesh", error = %e, "mesh identity unavailable");
                    return None;
                }
            };
            match vox_mesh_transport::bind(sk).await {
                Ok(ep) => Some(ep),
                Err(e) => {
                    tracing::warn!(target: "vox.workflow.mesh", error = %e, "mesh endpoint bind failed");
                    None
                }
            }
        })
        .await
        .as_ref()
}

/// Trusted peers that answered a `Probe`. Empty on any failure — an activity
/// must never fail because a peer is switched off.
#[cfg(feature = "mens")]
async fn probed_peers() -> Vec<Value> {
    let Some(ep) = mesh_endpoint().await else {
        return Vec::new();
    };
    let trust = std::sync::Arc::new(mesh_trust());
    vox_mesh_transport::directory(ep, &trust)
        .await
        .into_iter()
        .map(|p| {
            json!({
                "endpoint_id": p.endpoint_id.to_string(),
                "label": p.label,
                "host_triple": p.host_triple,
                "vox": p.vox,
            })
        })
        .collect()
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

#[cfg(all(test, feature = "mens"))]
mod tests {
    use super::*;

    fn activity(op: PopuliHttpOp) -> PopuliActivity {
        PopuliActivity {
            name: "mesh_thing".into(),
            populi_op: op,
            timeout_ms: None,
            activity_id: "act-1".into(),
            required_labels: None,
            is_detached: false,
        }
    }

    // `event` / `activity` / `activity_id` / `mesh_op` are an observable
    // contract of the `activity` language surface. A port that renames them
    // breaks user workflows, so pin them for every mesh-plane op.
    #[test]
    fn every_mesh_envelope_keeps_the_contract_keys() {
        for op in [
            PopuliHttpOp::Noop,
            PopuliHttpOp::Join,
            PopuliHttpOp::Snapshot,
            PopuliHttpOp::Heartbeat,
        ] {
            let v = mesh_envelope(&activity(op), "whatever", json!({}));
            assert_eq!(v["event"], "MeshActivity");
            assert_eq!(v["activity"], "mesh_thing");
            assert_eq!(v["activity_id"], "act-1");
            assert_eq!(v["mesh_op"], populi_op_json(op));
        }
    }

    #[test]
    fn extra_fields_merge_into_the_envelope() {
        let v = mesh_envelope(
            &activity(PopuliHttpOp::Snapshot),
            "snapshot_ok",
            json!({"node_count": 3}),
        );
        assert_eq!(v["control"], "snapshot_ok");
        assert_eq!(v["node_count"], 3);
    }

    // On iroh there is no control plane to register with: membership *is*
    // pairing, and reachability is demonstrated by a probe. Emitting `join_ok`
    // or `heartbeat_ok` would claim an acknowledgement nobody sent.
    #[test]
    fn join_and_heartbeat_never_claim_a_control_plane_ack() {
        for op in [PopuliHttpOp::Join, PopuliHttpOp::Heartbeat] {
            let v = mesh_envelope(&activity(op), mesh_control_label(op), json!({}));
            let control = v["control"].as_str().unwrap();
            assert_ne!(control, "join_ok");
            assert_ne!(control, "heartbeat_ok");
        }
    }

    // The mesh probes and does not execute: `ProbeOnlyExecutor` refuses `Run`
    // because no sandbox exists. With no HTTP control plane configured there is
    // no honest way to run a dispatch, so the step must fail loudly rather than
    // return the old `local_registry_only` success envelope.
    #[test]
    fn dispatch_without_an_execution_plane_is_an_error_that_names_the_reason() {
        for op in [PopuliHttpOp::Dispatch, PopuliHttpOp::Wait] {
            let e = no_execution_plane(&activity(op), op).to_string();
            assert!(e.contains("mesh_thing"), "must name the activity: {e}");
            assert!(e.contains("cannot execute"), "must name the reason: {e}");
            assert!(
                e.contains("VOX_MESH_CONTROL_ADDR"),
                "must name the actionable fix: {e}"
            );
        }
    }
}
