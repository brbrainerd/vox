//! Background remote worker loop for Populi `remote_task_envelope` rows.

use std::sync::Arc;
use std::sync::Mutex;

use crate::process_util::quiet_command;

use tracing::Instrument as _;

use super::envelope::{
    REMOTE_TASK_ENVELOPE_TYPE, REMOTE_TASK_RESULT_TYPE, RemoteTaskEnvelope, RemoteTaskResult,
};

const DEFAULT_POPULI_RECEIVER_AGENT: u64 = 2;
const DEFAULT_POPULI_SENDER_AGENT: u64 = 1;
const DEFAULT_MESH_WORKER_NODE_ID: &str = "vox-orch-worker";

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
/// Outcome of running a dispatched `.vox` source on the worker (Track B).
struct DispatchedExecParts {
    success: bool,
    result: Option<String>,
    error: Option<String>,
}

/// Build the legacy echo result (back-compat / `no-exec` policy): acknowledges
/// the payload byte count without executing anything.
fn echo_result(envelope: &RemoteTaskEnvelope) -> RemoteTaskResult {
    RemoteTaskResult {
        idempotency_key: envelope.idempotency_key.clone(),
        task_id: Some(envelope.task_id),
        success: true,
        result: Some(format!(
            "remote worker accepted payload ({} bytes)",
            envelope.payload.len()
        )),
        error: None,
    }
}

/// Execute a dispatched `.vox` source on this worker.
///
/// Returns `None` when execution is declined by node policy
/// (`VoxMeshExecPolicy == "no-exec"`) so the caller falls back to [`echo_result`].
/// Otherwise returns the real execution outcome — including refusals (invalid
/// base64, or a missing/mismatched BLAKE3 integrity hash) as `success: false`
/// **without spawning**. Source-only (`.vox` text); mirrors the audited executor
/// in `vox-populi` `transport::handlers::dispatch` (10 MiB output truncation),
/// run via the always-available `vox run --mode interp`. Reaching this path
/// already implies the poller's
/// `populi_remote_execute_experimental` gate is on.
///
/// SECURITY: this runs attacker-influenced code with the worker's privileges,
/// behind the experimental gate, a non-`no-exec` policy, and mandatory integrity
/// verification. It does NOT add sandboxing beyond what `vox run` provides, and
/// does NOT inject forwarded secrets into the subprocess (deferred — sandbox
/// tiering). Bundle/native execution stays out of this path.
/// Curated host env preserved across `env_clear()` so `vox`/the runtime still
/// works; everything else (including the worker's OWN secrets) is stripped from
/// the dispatched subprocess. Only keys present in the worker env are passed.
fn baseline_passthrough_env() -> Vec<(String, String)> {
    const KEYS: &[&str] = &[
        "PATH",
        "HOME",
        "TMPDIR",
        "TMP",
        "TEMP",
        "USER",
        "LOGNAME",
        "LANG",
        "LC_ALL",
        "LD_LIBRARY_PATH",
        "DYLD_LIBRARY_PATH",
        // Windows essentials:
        "SystemRoot",
        "windir",
        "SystemDrive",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "PATHEXT",
        "ComSpec",
        "NUMBER_OF_PROCESSORS",
        "PROCESSOR_ARCHITECTURE",
        "OS",
    ];
    KEYS.iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| ((*k).to_string(), v)))
        .collect()
}

/// Harden a dispatched subprocess: `env_clear()` (so dispatched code can't read
/// the worker's own environment / secrets), restore the curated baseline, then
/// add the tier-gated forwarded secrets (empty for BareMetal).
fn harden_dispatch_env(cmd: &mut std::process::Command, secret_env: &[(String, String)]) {
    cmd.env_clear();
    for (k, v) in baseline_passthrough_env() {
        cmd.env(k, v);
    }
    for (k, v) in secret_env {
        cmd.env(k, v);
    }
}

/// Extract the task's declared `required_secrets` (SecretId names) from the
/// envelope's `capability_requirements_json`. Empty when absent/unparseable.
fn parse_required_secrets(capability_requirements_json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(capability_requirements_json) else {
        return Vec::new();
    };
    let Some(secrets) = value.get("required_secrets").and_then(|s| s.as_array()) else {
        return Vec::new();
    };
    secrets
        .iter()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

fn parse_populi_agent_id(raw: Option<&str>, default: u64) -> u64 {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(default)
}

fn mesh_worker_node_id() -> String {
    match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshNodeId).expose() {
        Some(id) if !id.trim().is_empty() => id.to_string(),
        _ => DEFAULT_MESH_WORKER_NODE_ID.to_string(),
    }
}

fn run_dispatched_source(
    source_b64: &str,
    expected_blake3_hex: Option<&str>,
    policy: &str,
    secret_env: &[(String, String)],
) -> Option<DispatchedExecParts> {
    if policy == "no-exec" {
        return None;
    }
    let refuse = |error: String| {
        Some(DispatchedExecParts {
            success: false,
            result: None,
            error: Some(error),
        })
    };

    let source_bytes = match base64::engine::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        source_b64,
    ) {
        Ok(b) => b,
        Err(e) => return refuse(format!("invalid base64 source: {e}")),
    };
    // Integrity is mandatory for any source we are about to execute.
    let Some(expected_hex) = expected_blake3_hex else {
        return refuse("exec_source_blake3_hex is required to execute dispatched source".into());
    };
    let actual_hex = blake3::hash(&source_bytes).to_hex().to_string();
    if actual_hex != expected_hex {
        return refuse(format!(
            "integrity error: source hash mismatch (expected {expected_hex}, got {actual_hex})"
        ));
    }

    let tmp_file = std::env::temp_dir().join(format!(
        "vox-dispatch-{}.vox",
        vox_foundation::primitives::id::simple_hex_id()
    ));
    if let Err(e) = std::fs::write(&tmp_file, &source_bytes) {
        return refuse(format!("failed to write dispatch tmp file: {e}"));
    }
    // Use the always-available interpreter (`--mode interp`) rather than
    // `--mode script` (which requires a `script-execution` feature build), so a
    // worker can run dispatched source without a specially-built `vox` binary.
    let mut cmd = quiet_command("vox");
    cmd.arg("run").arg("--mode").arg("interp").arg(&tmp_file);
    harden_dispatch_env(&mut cmd, secret_env);
    let output = cmd.output();
    let _ = std::fs::remove_file(&tmp_file);

    Some(parts_from_output(output))
}

/// Which dispatched-bundle kind a byte blob is, by leading magic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BundleKind {
    /// WebAssembly module (`\0asm` magic) — runs under the wasmtime/WASI sandbox.
    Wasm,
    /// Anything else: a native executable.
    Native,
}

/// Classify a decoded bundle by leading magic. WASM modules start with `\0asm`.
fn classify_bundle(bytes: &[u8]) -> BundleKind {
    if bytes.starts_with(b"\0asm") {
        BundleKind::Wasm
    } else {
        BundleKind::Native
    }
}

/// Turn a finished (or failed) subprocess into [`DispatchedExecParts`], applying
/// the 10 MiB combined-output truncation shared by the source and bundle paths.
fn parts_from_output(output: std::io::Result<std::process::Output>) -> DispatchedExecParts {
    match output {
        Ok(out) => {
            const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;
            let mut stdout = out.stdout;
            let mut stderr = out.stderr;
            if stdout.len() + stderr.len() > MAX_OUTPUT_BYTES {
                if stderr.len() > MAX_OUTPUT_BYTES / 2 {
                    stderr.truncate(MAX_OUTPUT_BYTES / 2);
                }
                let remaining = MAX_OUTPUT_BYTES.saturating_sub(stderr.len());
                if stdout.len() > remaining {
                    stdout.truncate(remaining);
                }
            }
            let combined =
                String::from_utf8_lossy(&stdout).into_owned() + &String::from_utf8_lossy(&stderr);
            let success = out.status.success();
            DispatchedExecParts {
                success,
                result: Some(combined),
                error: if success {
                    None
                } else {
                    Some(format!("Exit code: {:?}", out.status.code()))
                },
            }
        }
        Err(e) => DispatchedExecParts {
            success: false,
            result: None,
            error: Some(format!("failed to spawn `vox`: {e}")),
        },
    }
}

/// Execute a dispatched precompiled **bundle** (WASM module or native binary) on
/// this worker.
///
/// Returns `None` when declined by `VoxMeshExecPolicy == "no-exec"` (caller
/// echoes). Otherwise returns the real outcome — including refusals (invalid
/// base64, missing/mismatched BLAKE3 integrity, `source-only` policy, or a native
/// binary under a non-`permissive` policy) as `success: false` **without
/// spawning**. WASM modules run under the wasmtime/WASI sandbox via `vox wasm run`
/// (the raw-`.wasm` runner, always available — not feature-gated); native binaries
/// execute directly and are therefore gated to `permissive` only. Mirrors the
/// control-plane executor in `vox-populi` `transport::handlers::dispatch`.
///
/// SECURITY: precompiled-code execution. WASM is sandboxed by wasmtime/WASI;
/// native execution is the most dangerous lane and is allowed only under an
/// explicit `permissive` policy. Integrity (BLAKE3) is mandatory. Forwarded-secret
/// injection remains deferred (sandbox tiering).
fn run_dispatched_bundle(
    bundle_b64: &str,
    expected_blake3_hex: Option<&str>,
    policy: &str,
    declared: &[String],
    secret_bag: Option<&crate::a2a::secret_bag::SecretBag>,
) -> Option<DispatchedExecParts> {
    if policy == "no-exec" {
        return None;
    }
    let refuse = |error: String| {
        Some(DispatchedExecParts {
            success: false,
            result: None,
            error: Some(error),
        })
    };
    if policy == "source-only" {
        return refuse(
            "node policy is source-only: precompiled bundle execution is disabled".into(),
        );
    }

    let bytes = match base64::engine::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        bundle_b64,
    ) {
        Ok(b) => b,
        Err(e) => return refuse(format!("invalid base64 bundle: {e}")),
    };
    let Some(expected_hex) = expected_blake3_hex else {
        return refuse("exec_bundle_blake3_hex is required to execute a dispatched bundle".into());
    };
    let actual_hex = blake3::hash(&bytes).to_hex().to_string();
    if actual_hex != expected_hex {
        return refuse(format!(
            "integrity error: bundle hash mismatch (expected {expected_hex}, got {actual_hex})"
        ));
    }

    let kind = classify_bundle(&bytes);
    if kind == BundleKind::Native && policy != "permissive" {
        return refuse(
            "node policy refuses native-binary bundles; only WASM bundles run unless \
             VoxMeshExecPolicy=permissive"
                .into(),
        );
    }

    let ext = if kind == BundleKind::Wasm {
        ".wasm"
    } else {
        ""
    };
    let tmp_file = std::env::temp_dir().join(format!(
        "vox-bundle-{}{}",
        vox_foundation::primitives::id::simple_hex_id(),
        ext
    ));
    if let Err(e) = std::fs::write(&tmp_file, &bytes) {
        return refuse(format!("failed to write bundle tmp file: {e}"));
    }
    #[cfg(unix)]
    if kind == BundleKind::Native {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp_file, std::fs::Permissions::from_mode(0o755));
    }

    let output = match kind {
        // WASM runs under the wasmtime/WASI sandbox via `vox wasm run`. This is a
        // REAL isolation tier (ExecTier::Sandboxed), so tier-gated low-value
        // secrets are forwarded — but ONLY as explicit `--env KEY=VALUE` args so
        // they reach the guest's WasmExecOpts.env; the subprocess env stays
        // hardened (no secrets there). Credentials are filtered by the gate.
        BundleKind::Wasm => {
            let wasm_secrets = match secret_bag {
                Some(bag) => crate::a2a::secret_gate::gate_secrets(
                    crate::a2a::secret_gate::ExecTier::Sandboxed,
                    declared,
                    bag,
                ),
                None => Vec::new(),
            };
            let mut cmd = quiet_command("vox");
            cmd.arg("wasm").arg("run").arg(&tmp_file);
            for (k, v) in &wasm_secrets {
                cmd.arg("--env").arg(format!("{k}={v}"));
            }
            harden_dispatch_env(&mut cmd, &[]);
            cmd.output()
        }
        // Native binary — BareMetal (no isolation): execute directly, forward
        // nothing (env hardened, no secrets).
        BundleKind::Native => {
            let mut cmd = quiet_command(&tmp_file);
            harden_dispatch_env(&mut cmd, &[]);
            cmd.output()
        }
    };
    let _ = std::fs::remove_file(&tmp_file);

    Some(parts_from_output(output))
}

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
    // `secret_bag` is consumed below by the trust-tier gate (secret_gate). Under
    // the only live tier (BareMetal) it forwards nothing, but the dispatched
    // subprocess env is hardened regardless (env_clear + curated baseline).

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

    // When the envelope carries executable content (and node policy permits),
    // run it and return real stdout/exit: `.vox` source via the interpreter
    // (Track B), or a precompiled WASM/native bundle via the wasmtime/WASI
    // isolation tier. Precedence: source, then bundle, then the legacy echo
    // (no exec content, or `VoxMeshExecPolicy == "no-exec"`).
    let result_payload = {
        let policy = match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshExecPolicy)
            .expose()
        {
            Some(p) if !p.trim().is_empty() => p.to_string(),
            _ => "permissive".to_string(),
        };

        // Trust-tier gate for forwarded secrets. Source + native bundle exec are
        // BareMetal (no isolation), so the gate forwards NOTHING today; the env is
        // hardened regardless. `declared` is the task's required_secrets.
        let declared = parse_required_secrets(&envelope.capability_requirements_json);
        let secret_env = match secret_bag.as_ref() {
            Some(bag) => crate::a2a::secret_gate::gate_secrets(
                crate::a2a::secret_gate::ExecTier::BareMetal,
                &declared,
                bag,
            ),
            None => Vec::new(),
        };

        let executed = if let Some(source_b64) = envelope.exec_source_b64.as_deref() {
            run_dispatched_source(
                source_b64,
                envelope.exec_source_blake3_hex.as_deref(),
                &policy,
                &secret_env,
            )
            .map(|parts| ("source", parts))
        } else if let Some(bundle_b64) = envelope.exec_bundle_b64.as_deref() {
            // The bundle runner gates per-kind internally (WASM ⇒ Sandboxed
            // forwarding, native ⇒ BareMetal/none), so it needs the declared list
            // + the bag rather than a precomputed BareMetal env.
            run_dispatched_bundle(
                bundle_b64,
                envelope.exec_bundle_blake3_hex.as_deref(),
                &policy,
                &declared,
                secret_bag.as_ref(),
            )
            .map(|parts| ("bundle", parts))
        } else {
            None
        };

        match executed {
            Some((kind, parts)) => {
                tracing::info!(
                    task_id = envelope.task_id,
                    success = parts.success,
                    kind,
                    "populi remote worker: executed dispatched payload"
                );
                RemoteTaskResult {
                    idempotency_key: envelope.idempotency_key.clone(),
                    task_id: Some(envelope.task_id),
                    success: parts.success,
                    result: parts.result,
                    error: parts.error,
                }
            }
            None => echo_result(&envelope),
        }
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

    let deliver_req = vox_populi::transport::A2ADeliverRequest {
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
    };
    // Mesh first when this node has a mesh identity and a trusted, addressable
    // peer (plan Task 3.1). The mailbox queues durably before it dials, so a
    // peer that is switched off is a delay rather than the delivery failure the
    // HTTP call below turns it into. HTTP stays as the fallback; deleting it is
    // Phase 6, after this is proven on two machines.
    let deliver_res: Result<(), ()> = if crate::a2a::mesh_relay::try_relay(&deliver_req).await {
        Ok(())
    } else {
        client.relay_a2a(&deliver_req).await.map(|_| ()).map_err(|_| ())
    };
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
    // Carry any mail queued for a peer that was off last tick. Nothing arrives
    // to trigger this retry on its own: the message is already ours to deliver.
    let flushed = crate::a2a::mesh_relay::flush_pending().await;
    if flushed > 0 {
        tracing::info!(flushed, "populi remote worker: delivered queued mesh mail");
    }

    let Ok(inbox) = client.relay_a2a_inbox(&receiver_agent.to_string()).await else {
        tracing::debug!(
            receiver_agent = receiver_agent,
            "populi remote worker: inbox HTTP failed"
        );
        return;
    };

    let node_id = mesh_worker_node_id();
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
    let Ok(mut guard) = join_slot.lock() else {
        tracing::error!("populi remote worker: join_slot mutex poisoned; poller not restarted");
        return;
    };
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
                        let receiver_agent = parse_populi_agent_id(
                            cfg.populi_remote_execute_receiver_agent.as_deref(),
                            DEFAULT_POPULI_RECEIVER_AGENT,
                        );
                        let sender_agent = parse_populi_agent_id(
                            cfg.populi_remote_execute_sender_agent.as_deref(),
                            DEFAULT_POPULI_SENDER_AGENT,
                        );
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
    let receiver_agent = parse_populi_agent_id(
        cfg.populi_remote_execute_receiver_agent.as_deref(),
        DEFAULT_POPULI_RECEIVER_AGENT,
    );
    let sender_agent = parse_populi_agent_id(
        cfg.populi_remote_execute_sender_agent.as_deref(),
        DEFAULT_POPULI_SENDER_AGENT,
    );
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
    use super::{
        BundleKind, classify_bundle, parse_remote_payload_context, run_dispatched_bundle,
        run_dispatched_source,
    };
    use serial_test::serial;

    fn b64(s: &str) -> String {
        base64::engine::Engine::encode(&base64::engine::general_purpose::STANDARD, s.as_bytes())
    }

    fn b64_bytes(b: &[u8]) -> String {
        base64::engine::Engine::encode(&base64::engine::general_purpose::STANDARD, b)
    }

    fn blake3_hex(s: &str) -> String {
        blake3::hash(s.as_bytes()).to_hex().to_string()
    }

    fn blake3_hex_bytes(b: &[u8]) -> String {
        blake3::hash(b).to_hex().to_string()
    }

    /// Minimal WASM module header: `\0asm` magic + version 1.
    const WASM_HEADER: &[u8] = b"\0asm\x01\0\0\0";

    /// `vox` reachable on PATH? Tests that actually spawn it skip cleanly when not.
    fn vox_on_path() -> bool {
        std::process::Command::new("vox")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn no_exec_policy_returns_none_for_echo_fallback() {
        let src = "pub fn main() { print(\"x\") }";
        let got = run_dispatched_source(&b64(src), Some(&blake3_hex(src)), "no-exec", &[]);
        assert!(got.is_none(), "no-exec policy must decline (caller echoes)");
    }

    #[test]
    fn missing_integrity_hash_refuses_without_spawning() {
        let src = "pub fn main() { print(\"x\") }";
        let parts = run_dispatched_source(&b64(src), None, "permissive", &[])
            .expect("a decision is returned");
        assert!(!parts.success);
        assert!(parts.result.is_none());
        assert!(
            parts.error.as_deref().unwrap_or("").contains("blake3"),
            "error should explain the missing integrity hash, got {:?}",
            parts.error
        );
    }

    #[test]
    fn hash_mismatch_refuses_without_spawning() {
        let src = "pub fn main() { print(\"x\") }";
        let parts = run_dispatched_source(&b64(src), Some("deadbeef"), "permissive", &[])
            .expect("a decision is returned");
        assert!(!parts.success);
        assert!(parts.result.is_none());
        assert!(
            parts
                .error
                .as_deref()
                .unwrap_or("")
                .contains("hash mismatch"),
            "error should report a hash mismatch, got {:?}",
            parts.error
        );
    }

    #[test]
    fn invalid_base64_refuses_without_spawning() {
        let parts = run_dispatched_source("!!!not base64!!!", Some("deadbeef"), "permissive", &[])
            .expect("a decision is returned");
        assert!(!parts.success);
        assert!(parts.error.as_deref().unwrap_or("").contains("base64"));
    }

    #[test]
    #[serial(vox_spawn)]
    fn executes_source_and_returns_real_stdout() {
        if !vox_on_path() {
            eprintln!("skipping: `vox` not on PATH");
            return;
        }
        let src = "pub fn main() {\n    print(\"hello from \" + str(2 + 3))\n}\n";
        let parts = run_dispatched_source(&b64(src), Some(&blake3_hex(src)), "permissive", &[])
            .expect("executed");
        assert!(parts.success, "expected success, error={:?}", parts.error);
        assert!(
            parts
                .result
                .as_deref()
                .unwrap_or("")
                .contains("hello from 5"),
            "expected real stdout, got {:?}",
            parts.result
        );
    }

    #[test]
    #[serial(vox_spawn)]
    fn built_exec_source_fields_round_trip_to_real_execution() {
        // End-to-end: the sender helper builds (b64, hash); the worker re-verifies
        // the hash and executes — proving the dispatch contract closes the loop.
        if !vox_on_path() {
            eprintln!("skipping: `vox` not on PATH");
            return;
        }
        let src = "pub fn main() {\n    print(\"answer \" + str(6 * 7))\n}\n";
        let (b64, hex) = crate::a2a::exec_source::build_exec_source_fields(src);
        let parts = run_dispatched_source(&b64, Some(&hex), "permissive", &[]).expect("executed");
        assert!(parts.success, "expected success, error={:?}", parts.error);
        assert!(
            parts.result.as_deref().unwrap_or("").contains("answer 42"),
            "expected real stdout from built source, got {:?}",
            parts.result
        );
    }

    #[test]
    #[serial(vox_spawn)]
    fn nonzero_exit_sets_success_false() {
        if !vox_on_path() {
            eprintln!("skipping: `vox` not on PATH");
            return;
        }
        // Not valid Vox — `vox run` exits non-zero.
        let src = "this is not valid vox source @@@";
        let parts = run_dispatched_source(&b64(src), Some(&blake3_hex(src)), "permissive", &[])
            .expect("executed");
        assert!(!parts.success, "a failing script must report success=false");
        assert!(parts.error.is_some());
    }

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

    // ── Bundle / WASM execution (mesh worker) ──────────────────────────────

    #[test]
    fn classify_bundle_detects_wasm_and_native() {
        assert_eq!(classify_bundle(WASM_HEADER), BundleKind::Wasm);
        assert_eq!(classify_bundle(b"\x7fELF not-really"), BundleKind::Native);
        assert_eq!(classify_bundle(b""), BundleKind::Native);
    }

    #[test]
    fn bundle_no_exec_policy_returns_none_for_echo_fallback() {
        let got = run_dispatched_bundle(
            &b64_bytes(WASM_HEADER),
            Some(&blake3_hex_bytes(WASM_HEADER)),
            "no-exec",
            &[],
            None,
        );
        assert!(got.is_none(), "no-exec policy must decline (caller echoes)");
    }

    #[test]
    fn bundle_source_only_policy_refuses_without_spawning() {
        let parts = run_dispatched_bundle(
            &b64_bytes(WASM_HEADER),
            Some(&blake3_hex_bytes(WASM_HEADER)),
            "source-only",
            &[],
            None,
        )
        .expect("a decision is returned");
        assert!(!parts.success);
        assert!(parts.error.as_deref().unwrap_or("").contains("source-only"));
    }

    #[test]
    fn bundle_missing_integrity_hash_refuses_without_spawning() {
        let parts = run_dispatched_bundle(&b64_bytes(WASM_HEADER), None, "permissive", &[], None)
            .expect("a decision is returned");
        assert!(!parts.success);
        assert!(parts.error.as_deref().unwrap_or("").contains("blake3"));
    }

    #[test]
    fn bundle_hash_mismatch_refuses_without_spawning() {
        let parts = run_dispatched_bundle(
            &b64_bytes(WASM_HEADER),
            Some("deadbeef"),
            "permissive",
            &[],
            None,
        )
        .expect("a decision is returned");
        assert!(!parts.success);
        assert!(
            parts
                .error
                .as_deref()
                .unwrap_or("")
                .contains("hash mismatch")
        );
    }

    #[test]
    fn native_bundle_refused_under_non_permissive_policy() {
        // A non-WASM blob under a non-permissive policy must be refused WITHOUT
        // ever executing a native binary.
        let native = b"\x7fELF\x02\x01\x01\0 not a real binary";
        let parts = run_dispatched_bundle(
            &b64_bytes(native),
            Some(&blake3_hex_bytes(native)),
            "strict",
            &[],
            None,
        )
        .expect("a decision is returned");
        assert!(!parts.success);
        assert!(
            parts.error.as_deref().unwrap_or("").contains("native"),
            "native bundles must be refused unless policy is permissive, got {:?}",
            parts.error
        );
    }

    /// A minimal but REAL WASI module: imports `proc_exit`, exports `memory` +
    /// `_start`, and exits 0. Unlike the 8-byte header, this actually runs under
    /// `vox run --isolation wasm`.
    fn minimal_wasi_module() -> Vec<u8> {
        wat::parse_str(
            r#"(module
              (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
              (memory (export "memory") 1)
              (func (export "_start")
                i32.const 0
                call $exit))"#,
        )
        .expect("valid WAT")
    }

    /// WASI module whose exit code == the number of env vars the guest sees
    /// (`environ_sizes_get` → `proc_exit(count)`). Lets the worker test assert how
    /// many secrets were forwarded into the sandbox.
    fn env_count_wasm() -> Vec<u8> {
        wat::parse_str(
            r#"(module
              (import "wasi_snapshot_preview1" "environ_sizes_get" (func $sz (param i32 i32) (result i32)))
              (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
              (memory (export "memory") 1)
              (func (export "_start")
                (drop (call $sz (i32.const 0) (i32.const 4)))
                (call $exit (i32.load (i32.const 0)))))"#,
        )
        .expect("valid WAT")
    }

    #[test]
    #[serial(vox_spawn)]
    fn wasm_bundle_forwards_low_value_secret_but_not_credential() {
        // End-to-end: a SecretBag with a low-value config (VoxOpenRouterChatModel)
        // and a credential (OpenRouterApiKey); the WASM (Sandboxed) lane must
        // forward ONLY the low-value one into the sandbox via `vox wasm run --env`.
        // The env-count module's exit code == #env vars the guest sees, so:
        //   both declared → guest sees 1 (credential filtered) → exit code 1.
        if !vox_on_path() {
            eprintln!("skipping: `vox` not on PATH");
            return;
        }
        let wasm = env_count_wasm();
        let bag = crate::a2a::secret_bag::SecretBag::from_decrypted(serde_json::json!({
            "VoxOpenRouterChatModel": "some/model",
            "OpenRouterApiKey": "sk-secret",
        }))
        .expect("bag");
        let declared = vec![
            "VoxOpenRouterChatModel".to_string(),
            "OpenRouterApiKey".to_string(),
        ];

        let parts = run_dispatched_bundle(
            &b64_bytes(&wasm),
            Some(&blake3_hex_bytes(&wasm)),
            "permissive",
            &declared,
            Some(&bag),
        )
        .expect("a decision is returned");

        // Exactly one env var crossed into the sandbox (the credential was filtered),
        // so the module exited with code 1.
        assert!(
            !parts.success && parts.error.as_deref().unwrap_or("").contains("Some(1)"),
            "exactly the 1 low-value secret must be forwarded (credential filtered); got success={} error={:?}",
            parts.success,
            parts.error
        );
    }

    #[test]
    #[serial(vox_spawn)]
    fn wasm_bundle_executes_via_wasm_run_to_clean_exit() {
        // Genuinely exercises the WASM lane end-to-end: a real WASI module run via
        // `vox wasm run` (the always-available raw-.wasm runner — NOT feature-gated).
        // Skips only when `vox` is absent from PATH; never passes vacuously.
        if !vox_on_path() {
            eprintln!("skipping: `vox` not on PATH");
            return;
        }
        let wasm = minimal_wasi_module();
        assert!(wasm.starts_with(b"\0asm"), "synthesized bytes must be WASM");

        let parts = run_dispatched_bundle(
            &b64_bytes(&wasm),
            Some(&blake3_hex_bytes(&wasm)),
            "permissive",
            &[],
            None,
        )
        .expect("a decision is returned (integrity + classification passed)");

        assert!(
            parts.success,
            "a valid WASI module must run to a clean exit via `vox wasm run`; error={:?}",
            parts.error
        );
    }
}
