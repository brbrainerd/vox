//! Script dispatch handlers: dispatch_script, dispatch_results_poll, queue_stats, execute_on_worker.

use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use base64::Engine as _;

use crate::{NodeRecord, node_maintenance_blocks_new_work};

use super::super::auth::{PopuliAuthContext, auth_allows_deliver, auth_allows_worker_plane};
#[cfg(feature = "transport")]
use super::super::dispatch_results_sweep;
use super::super::{DispatchRequest, DispatchResponse, MeshQueueStats, PopuliTransportState};
use super::nodes::ResponseErr;

pub(crate) async fn dispatch_script(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<DispatchRequest>,
) -> Result<Json<DispatchResponse>, ResponseErr> {
    if !auth_allows_deliver(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: submitter/mesh/admin token required for dispatch".into(),
        ));
    }

    let nodes = st.inner.read().await;
    let target = if let Some(id) = &req.node_id {
        nodes.nodes.iter().find(|n| n.id == *id).cloned()
    } else {
        select_best_node(&nodes.nodes, &req).cloned()
    };
    drop(nodes);

    let Some(target) = target else {
        return Err(ResponseErr(
            StatusCode::NOT_FOUND,
            "populi: no suitable worker node found for dispatch".into(),
        ));
    };

    let Some(addr) = &target.listen_addr else {
        return Err(ResponseErr(
            StatusCode::BAD_REQUEST,
            format!("populi: target node {} has no listen_addr", target.id),
        ));
    };

    // Forward to worker
    let client = crate::http_client::PopuliHttpClient::new(addr).with_env_token();

    if req.is_detached {
        use vox_foundation::primitives::id::simple_hex_id;
        let dispatch_id = simple_hex_id();
        let st_cl = st.clone();
        let dispatch_id_cl = dispatch_id.clone();
        let dispatch_id_for_store = dispatch_id.clone();
        let target_node_id = target.id.clone();

        tokio::spawn(async move {
            let res = client.worker_execute(&req).await;
            match res {
                Ok(mut resp) => {
                    resp.expires_unix_ms = Some(crate::now_ms() + 3_600_000); // 1 hour TTL
                    st_cl.dispatch_results.insert(dispatch_id_cl, resp);
                }
                Err(e) => {
                    st_cl.dispatch_results.insert(
                        dispatch_id_cl,
                        DispatchResponse {
                            success: false,
                            output: String::new(),
                            error: Some(format!(
                                "populi: detached execution failed to forward: {}",
                                e
                            )),
                            node_id: target_node_id,
                            duration_ms: 0,
                            exit_code: None,
                            is_truncated: false,
                            expires_unix_ms: Some(crate::now_ms() + 3_600_000),
                        },
                    );
                }
            }
            if let Some(path) = &st_cl.dispatch_results_store_path {
                let _ = super::super::store::persist_dispatch_results_store(
                    path,
                    &st_cl.dispatch_results,
                );
            }
            if let Some(ms) = st_cl.mesh_store.clone()
                && let Some(val) = st_cl.dispatch_results.get(&dispatch_id_for_store)
            {
                let key = dispatch_id_for_store.clone();
                let result = val.clone();
                tokio::spawn(async move {
                    if let Err(e) = ms.put_dispatch_result(&key, &result).await {
                        tracing::warn!(error = %e, key, "mesh_store put_dispatch_result failed");
                    }
                });
            }
        });

        Ok(Json(DispatchResponse {
            success: true,
            output: format!(
                "populi: detached dispatch accepted. poll for results with id: {}",
                dispatch_id
            ),
            error: None,
            node_id: target.id,
            duration_ms: 0,
            exit_code: None,
            is_truncated: false,
            expires_unix_ms: None,
        }))
    } else {
        let mut resp = client.worker_execute(&req).await.map_err(|e| {
            ResponseErr(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("populi: failed to forward dispatch to worker: {}", e),
            )
        })?;
        resp.expires_unix_ms = None;
        Ok(Json(resp))
    }
}

pub(crate) async fn dispatch_results_poll(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    axum::extract::Path(dispatch_id): axum::extract::Path<String>,
) -> Result<Json<DispatchResponse>, ResponseErr> {
    if !auth_allows_deliver(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: submitter/mesh/admin token required for dispatch polls".into(),
        ));
    }

    #[cfg(feature = "transport")]
    dispatch_results_sweep(&st.dispatch_results, crate::now_ms());

    if let Some(res) = st.dispatch_results.get(&dispatch_id) {
        Ok(Json(res.clone()))
    } else {
        Err(ResponseErr(
            StatusCode::NOT_FOUND,
            format!(
                "populi: dispatch result for id {} not found or still in-flight",
                dispatch_id
            ),
        ))
    }
}

fn select_best_node<'a>(nodes: &'a [NodeRecord], req: &DispatchRequest) -> Option<&'a NodeRecord> {
    let mut candidates: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.quarantined != Some(true) && !node_maintenance_blocks_new_work(crate::now_ms(), n)
        })
        .filter(|n| {
            // Label matching
            if let Some(required) = &req.required_labels
                && !required.is_empty()
                && !required
                    .iter()
                    .all(|req_lab| n.capabilities.labels.contains(req_lab))
            {
                return false;
            }
            // VRAM matching
            if let Some(min_vram) = req.min_vram_mb {
                let node_vram = n.capabilities.min_vram_mb.unwrap_or(0);
                if node_vram < min_vram {
                    return false;
                }
            }
            // Donation policy matching
            if let (Some(task_kind_str), Some(policy)) = (&req.task_kind, &n.donation_policy) {
                let allowed = policy.slots.iter().any(|slot| {
                    format!("{:?}", slot.task_kind).to_lowercase() == task_kind_str.to_lowercase()
                });
                if !allowed {
                    return false;
                }
            }
            true
        })
        .collect();

    // Load balancing: Sort by CPU usage ascending
    candidates.sort_by(|a, b| {
        let a_usage = a.cpu_usage_pct.unwrap_or(100.0);
        let b_usage = b.cpu_usage_pct.unwrap_or(100.0);
        a_usage
            .partial_cmp(&b_usage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    candidates.first().copied()
}

pub(crate) async fn queue_stats(
    State(st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
) -> Result<Json<MeshQueueStats>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for queue stats".into(),
        ));
    }
    let msgs = st.a2a_messages.read().await;
    let mut stats = MeshQueueStats {
        pending_count: 0,
        pending_by_kind: std::collections::HashMap::new(),
        pending_by_priority: std::collections::HashMap::new(),
    };

    for m in msgs.iter() {
        if m.acknowledged || m.lease_holder_node_id.is_some() {
            continue;
        }
        stats.pending_count += 1;
        if let Some(kind) = &m.task_kind {
            *stats.pending_by_kind.entry(kind.clone()).or_insert(0) += 1;
        }
        *stats.pending_by_priority.entry(m.priority).or_insert(0) += 1;
    }

    Ok(Json(stats))
}

pub(crate) async fn execute_on_worker(
    State(_st): State<PopuliTransportState>,
    Extension(ctx): Extension<PopuliAuthContext>,
    Json(req): Json<DispatchRequest>,
) -> Result<Json<DispatchResponse>, ResponseErr> {
    if !auth_allows_worker_plane(&ctx) {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi: worker/mesh/admin token required for worker execution".into(),
        ));
    }

    // Phase 4: Policy Gating
    let secret = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshExecPolicy);
    let policy = secret.expose().unwrap_or("permissive");
    if req.is_bundle && policy == "source-only" {
        return Err(ResponseErr(
            StatusCode::FORBIDDEN,
            "populi policy: this node only allows source-based dispatch (binary execution disabled)".into(),
        ));
    }

    if let Some(req_labels) = &req.required_labels {
        let local_record = crate::node_record_for_current_process("".into(), None);
        for req_label in req_labels {
            if !local_record.capabilities.labels.contains(req_label) {
                return Err(ResponseErr(
                    StatusCode::FORBIDDEN,
                    format!(
                        "populi capacity constraints: this node lacks the required capability label '{}'",
                        req_label
                    ),
                ));
            }
        }
    }

    let source_bytes = base64::engine::general_purpose::STANDARD
        .decode(&req.source)
        .map_err(|e| {
            ResponseErr(
                StatusCode::BAD_REQUEST,
                format!("populi: invalid base64 source: {}", e),
            )
        })?;

    // Phase 2: Integrity Verification
    if let Some(expected_hex) = &req.source_blake3_hex {
        let actual_hash = blake3::hash(&source_bytes);
        let actual_hex = actual_hash.to_hex().to_string();
        if &actual_hex != expected_hex {
            return Err(ResponseErr(
                StatusCode::BAD_REQUEST,
                format!(
                    "populi integrity error: bundle hash mismatch (expected {}, got {})",
                    expected_hex, actual_hex
                ),
            ));
        }
    }

    let tmp_dir = std::env::temp_dir();
    let bin_path = if req.is_bundle {
        // Source is actually a pre-compiled binary.
        // Identify .wasm vs native.
        let is_wasm = source_bytes.starts_with(b"\0asm");
        let ext = if is_wasm { ".wasm" } else { "" };
        let file_name = format!("vox-bundle-{}{}", vox_foundation::primitives::id::simple_hex_id(), ext);
        let tmp_file = tmp_dir.join(file_name);
        std::fs::write(&tmp_file, &source_bytes).map_err(|e| {
            ResponseErr(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("populi: failed to write bundle: {}", e),
            )
        })?;

        // Ensure executable on Unix
        #[cfg(unix)]
        if !is_wasm {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&tmp_file)
                .map(|m| m.permissions())
                .unwrap_or_else(|_| std::fs::Permissions::from_mode(0o755));
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&tmp_file, perms);
        }

        tmp_file
    } else {
        let file_name = format!("vox-dispatch-{}.vox", vox_foundation::primitives::id::simple_hex_id());
        let tmp_file = tmp_dir.join(file_name);
        std::fs::write(&tmp_file, &source_bytes).map_err(|e| {
            ResponseErr(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("populi: failed to write tmp file: {}", e),
            )
        })?;
        tmp_file
    };

    let start_time = std::time::Instant::now();

    let output = if req.is_bundle {
        if bin_path.extension().is_some_and(|ext| ext == "wasm") {
            std::process::Command::new("vox")
                .arg("run")
                .arg("--mode")
                .arg("script")
                .arg("--isolation")
                .arg("wasm")
                .arg(&bin_path)
                .output()
        } else {
            std::process::Command::new(&bin_path).output()
        }
    } else {
        std::process::Command::new("vox")
            .arg("run")
            .arg("--mode")
            .arg("script")
            .arg(&bin_path)
            .output()
    };

    let duration_ms = start_time.elapsed().as_millis() as u64;

    // Cleanup early
    let _ = std::fs::remove_file(&bin_path);

    match output {
        Ok(out) => {
            // Phase 3: Output Truncation (10MB Limit)
            const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
            let mut combined_stdout = out.stdout;
            let mut combined_stderr = out.stderr;

            let total_len = combined_stdout.len() + combined_stderr.len();
            let is_truncated = total_len > MAX_OUTPUT_BYTES;

            if is_truncated {
                // Keep the first 10MB of stderr then stdout or split evenly
                if combined_stderr.len() > MAX_OUTPUT_BYTES / 2 {
                    combined_stderr.truncate(MAX_OUTPUT_BYTES / 2);
                }
                let remaining = MAX_OUTPUT_BYTES.saturating_sub(combined_stderr.len());
                if combined_stdout.len() > remaining {
                    combined_stdout.truncate(remaining);
                }
            }

            let output_str = String::from_utf8_lossy(&combined_stdout).to_string()
                + &String::from_utf8_lossy(&combined_stderr);

            Ok(Json(DispatchResponse {
                success: out.status.success(),
                output: output_str,
                is_truncated,
                duration_ms,
                exit_code: out.status.code(),
                error: if out.status.success() {
                    None
                } else {
                    Some(format!("Exit code: {:?}", out.status.code()))
                },
                node_id: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
                    .expose()
                    .unwrap_or("unknown")
                    .to_string(),
                expires_unix_ms: None,
            }))
        }
        Err(e) => Ok(Json(DispatchResponse {
            success: false,
            output: String::new(),
            is_truncated: false,
            duration_ms,
            exit_code: None,
            error: Some(format!("Failed to execute vox: {}", e)),
            node_id: vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId)
                .expose()
                .unwrap_or("unknown")
                .to_string(),
            expires_unix_ms: None,
        })),
    }
}
