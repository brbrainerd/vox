//! Compiler, build, lint, and test tool handlers for the Vox MCP server.
//!
//! Covers: validate_file, run_tests, check_workspace, test_all, build_crate,
//! lint_crate, coverage_report, generate_vox_code.
//!
//! Subprocess and file reads use Tokio async I/O so `super::handle_tool_call` does not block
//! the runtime. TOESTUB runs inside [`tokio::task::spawn_blocking`] because the engine is synchronous.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::speech_constraints;
use crate::params::{RunTestsParams, ToolResult};
use crate::server_state::ServerState;

const REM_CARGO_DISABLED: &str = "Bind the MCP server to a Cargo workspace or package root, or use repository capabilities that enable Cargo tools.";
const REM_CARGO_TEST: &str = "Read STDOUT/STDERR for failing tests; run `cargo test` locally with the same filter and fix code or env.";
const REM_CARGO_SPAWN: &str = "Ensure `cargo` is installed and on PATH for the MCP process (see build-environment docs for agent shells).";
const REM_CARGO_CHECK: &str = "Fix compiler errors shown in stderr; run `cargo check --workspace` locally for full diagnostics.";
const REM_CARGO_BUILD: &str = "Fix build errors in stderr; verify features, targets, and that no concurrent build holds file locks.";
const REM_COVERAGE: &str =
    "Install `cargo-llvm-cov` (`cargo install cargo-llvm-cov`) or run coverage outside MCP.";
const REM_GEN_PROMPT: &str = "Provide a non-empty `prompt` describing the `.vox` code to generate.";
const REM_GEN_OUTPUT_PATH: &str = "Use a workspace-relative path without `..`; bind MCP to the repository root; parent directories are created automatically.";
const REM_MCP_MODEL_LOCK: &str =
    "Retry; restart the MCP server if `mcp_chat_model_override` stays poisoned.";
const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";
const REM_CODEGEN_REPAIR: &str = "Simplify the ask, paste compiler errors explicitly, lower constraints, or set `validate:false` for a raw draft.";
const REM_CODEGEN_STALL: &str = "Diagnostics did not change across retries — rephrase the prompt or disable validation temporarily.";
const RUNTIME_GENERATION_KPI_SCHEMA: &str = "vox_runtime_generation_kpi_v1";

async fn write_codegen_utf8_under_repo(
    repo_root: &Path,
    user_rel: &str,
    utf8: &str,
) -> Result<(PathBuf, String, usize), String> {
    let dest =
        vox_repository::resolve_strict_repo_relative_path(repo_root, user_rel).map_err(|e| {
            if e.contains("empty") {
                "output_path must not be empty".to_string()
            } else if e.contains("relative") {
                "output_path must be relative to the repository root".to_string()
            } else if e.contains("..") {
                "output_path must not contain '..'".to_string()
            } else {
                e
            }
        })?;
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create_dir_all {}: {e}", parent.display()))?;
    }
    let bytes = utf8.as_bytes();
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| format!("write {}: {e}", dest.display()))?;
    let rel = vox_repository::path_relative_to_repo_root(repo_root, &dest)
        .map_err(|e| format!("refusal: output path resolves outside repository root ({e})"))?;
    Ok((dest, rel, bytes.len()))
}

fn merge_file_outcomes_meta(
    mut meta: serde_json::Value,
    rel_path: String,
    bytes_written: usize,
    post_write_snapshot_id: Option<&str>,
) -> serde_json::Value {
    let mut outcomes = serde_json::json!({
        "schema": "vox_generate_code_file_outcomes_v1",
        "schema_version": 1,
        "written_paths": [rel_path],
        "bytes_written": bytes_written,
        "artifact_kind": "vox_generated",
    });
    if let Some(sid) = post_write_snapshot_id {
        if let Some(o) = outcomes.as_object_mut() {
            o.insert(
                "post_write_snapshot_id".into(),
                serde_json::Value::String(sid.to_string()),
            );
        }
    }
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("file_outcomes".into(), outcomes);
    }
    meta
}

async fn merge_meta_after_codegen_write(
    state: &ServerState,
    meta: serde_json::Value,
    output_rel: &str,
    utf8: &str,
    vcs_agent_id: Option<u64>,
) -> Result<serde_json::Value, String> {
    let (dest, rel, n) =
        write_codegen_utf8_under_repo(&state.repository.root, output_rel, utf8).await?;
    let snap = if let Some(aid) = vcs_agent_id {
        Some(
            state
                .orchestrator
                .capture_snapshot(
                    vox_orchestrator::types::AgentId(aid),
                    &[dest],
                    format!("vox_generate_code: {rel}"),
                )
                .await
                .to_string(),
        )
    } else {
        None
    };
    Ok(merge_file_outcomes_meta(meta, rel, n, snap.as_deref()))
}

fn runtime_generation_kpi(
    success: bool,
    validate_requested: bool,
    compile_pass: Option<bool>,
    canonical_pass: Option<bool>,
    attempts: u64,
    repair_stalled: bool,
    time_to_first_valid_ms: Option<u128>,
    output_tokens: u64,
    surface_contract_failures: u64,
    failure_category: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "schema": RUNTIME_GENERATION_KPI_SCHEMA,
        "schema_version": 1,
        "surface": "mcp",
        "success": success,
        "validate_requested": validate_requested,
        "compile_pass": compile_pass,
        "canonical_pass": canonical_pass,
        "attempts": attempts,
        "repair_stalled": repair_stalled,
        "time_to_first_valid_ms": time_to_first_valid_ms.map(|v| v as u64),
        "output_tokens": output_tokens,
        "surface_contract_failures": surface_contract_failures,
        "strictness_failures": surface_contract_failures,
        "canonicalization_failures": null,
        "failure_category": failure_category,
    })
}

async fn record_expensive_op(state: &ServerState) {
    if let Ok(mut sm) = state.session_manager.try_lock() {
        let sid = sm.list_sessions().first().map(|s| s.id.clone());
        if let Some(id) = sid {
            let _ = sm.record_expensive_op(&id);
        }
    }
}

fn cargo_unavailable_message(state: &ServerState) -> Option<String> {
    let c = &state.repository.capabilities;
    if c.cargo_workspace || c.cargo_package {
        None
    } else {
        Some(
            "Repository root is not a Cargo package or workspace; Cargo MCP tools are disabled for this repository."
                .to_string(),
        )
    }
}

pub async fn run_tests(state: &ServerState, params: RunTestsParams) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.current_dir(&state.repository.root);
    cmd.arg("test");
    if state.repository.capabilities.cargo_workspace {
        cmd.arg("-p").arg(&params.crate_name);
    }

    if let Some(filter) = &params.test_filter {
        cmd.arg(filter);
    }

    cmd.args(["--", "--nocapture"]);

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}");

            if output.status.success() {
                ToolResult::ok(combined).to_json()
            } else {
                ToolResult::<String>::err_with_remediation(combined, REM_CARGO_TEST).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("failed to run cargo test: {e}"),
            REM_CARGO_SPAWN,
        )
        .to_json(),
    }
}

/// Run `cargo check` for the entire workspace.
pub async fn check_workspace(state: &ServerState) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.current_dir(&state.repository.root);
    cmd.arg("check").arg("--message-format=short");
    if state.repository.capabilities.cargo_workspace {
        cmd.arg("--workspace");
    }

    match cmd.output().await {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                record_expensive_op(state).await;
                ToolResult::ok("workspace check passed".to_string()).to_json()
            } else {
                ToolResult::<String>::err_with_remediation(
                    format!("check failed:\n{stderr}"),
                    REM_CARGO_CHECK,
                )
                .to_json()
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("failed to run cargo check: {e}"),
            REM_CARGO_SPAWN,
        )
        .to_json(),
    }
}

/// Run `cargo test` for the entire workspace.
pub async fn test_all(state: &ServerState) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.current_dir(&state.repository.root);
    cmd.arg("test");
    if state.repository.capabilities.cargo_workspace {
        cmd.arg("--workspace");
    }

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("STDOUT:\n{stdout}\n\nSTDERR:\n{stderr}");

            if output.status.success() {
                record_expensive_op(state).await;
                ToolResult::ok(combined).to_json()
            } else {
                ToolResult::<String>::err_with_remediation(combined, REM_CARGO_TEST).to_json()
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("failed to run cargo test --workspace: {e}"),
            REM_CARGO_SPAWN,
        )
        .to_json(),
    }
}

/// Run `cargo build` for a crate or the whole workspace.
pub async fn build_crate(state: &ServerState, crate_name: Option<&str>) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.current_dir(&state.repository.root);
    cmd.arg("build");
    if let Some(c) = crate_name {
        cmd.args(["-p", c]);
    } else if state.repository.capabilities.cargo_workspace {
        cmd.arg("--workspace");
    }

    match cmd.output().await {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            if output.status.success() {
                record_expensive_op(state).await;
                ToolResult::ok(format!("Build succeeded.\n{stdout}")).to_json()
            } else {
                ToolResult::<String>::err_with_remediation(
                    format!("Build failed:\n{stderr}"),
                    REM_CARGO_BUILD,
                )
                .to_json()
            }
        }
        Err(e) => ToolResult::<String>::err_with_remediation(
            format!("failed to run cargo build: {e}"),
            REM_CARGO_SPAWN,
        )
        .to_json(),
    }
}

/// Run `cargo clippy` and TOESTUB for a crate or the whole workspace.
pub async fn lint_crate(state: &ServerState, crate_name: Option<&str>) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }

    let repo_root = state.repository.root.clone();
    let workspace = state.repository.capabilities.cargo_workspace;
    let crate_owned = crate_name.map(str::to_string);

    let mut clippy_cmd = tokio::process::Command::new("cargo");
    clippy_cmd.current_dir(&repo_root);
    clippy_cmd.arg("clippy");
    if let Some(ref c) = crate_owned {
        clippy_cmd.args(["-p", c.as_str()]);
    } else if workspace {
        clippy_cmd.arg("--workspace");
    }
    clippy_cmd.args(["--", "-D", "warnings"]);

    let clippy_out = match clippy_cmd.output().await {
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                "Clippy clean.".to_string()
            } else {
                format!("Clippy errors:\n{stderr}")
            }
        }
        Err(e) => format!("failed to run cargo clippy: {e}"),
    };

    let root_for_ts = if let Some(ref c) = crate_owned {
        let p = repo_root.join("crates").join(c);
        if tokio::fs::try_exists(&p).await.unwrap_or(false) {
            p
        } else {
            repo_root.clone()
        }
    } else {
        repo_root.clone()
    };

    use vox_code_audit::{Severity, ToestubConfig, ToestubEngine};

    let ts_report = match tokio::task::spawn_blocking(move || {
        let ts_config = ToestubConfig {
            roots: vec![root_for_ts],
            min_severity: Severity::Warning,
            schema_path: Some(PathBuf::from("vox-schema.json")),
            ..ToestubConfig::default()
        };
        let ts_engine = ToestubEngine::new(ts_config);
        let (_, report) = ts_engine.run_and_report();
        report
    })
    .await
    {
        Ok(r) => r,
        Err(e) => format!("TOESTUB task failed: {e}"),
    };

    let combined = format!(
        "### 📎 Clippy Results\n{}\n\n### 🦶 TOESTUB Architectural Scan\n{}",
        clippy_out, ts_report
    );

    record_expensive_op(state).await;
    ToolResult::ok(combined).to_json()
}

/// Run `cargo llvm-cov` for a text coverage summary.
pub async fn coverage_report(state: &ServerState, crate_name: Option<&str>) -> String {
    if let Some(msg) = cargo_unavailable_message(state) {
        return ToolResult::<String>::err_with_remediation(msg, REM_CARGO_DISABLED).to_json();
    }
    let mut cmd = tokio::process::Command::new("cargo");
    cmd.current_dir(&state.repository.root);
    cmd.arg("llvm-cov");
    if let Some(c) = crate_name {
        cmd.args(["-p", c]);
    }
    cmd.args(["--text"]);

    match cmd.output().await {
        Ok(output) if output.status.success() => {
            ToolResult::ok(String::from_utf8_lossy(&output.stdout).to_string()).to_json()
        }
        _ => ToolResult::<String>::err_with_remediation(
            "cargo-llvm-cov is not installed or failed. Run `cargo install cargo-llvm-cov` and ensure `rustup component add llvm-tools-preview`."
                .to_string(),
            REM_COVERAGE,
        )
        .to_json(),
    }
}

/// Generate validated Vox code from a prompt using the native LLM bridge.
pub async fn generate_vox_code(state: &ServerState, args: serde_json::Value) -> String {
    let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let session_id = args
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string);
    let journey_id = args
        .get("journey_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "journey_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            )
        });
    let validate_flag = args
        .get("validate")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let max_retries = args
        .get("max_retries")
        .and_then(|v| v.as_u64())
        .unwrap_or(2)
        .min(speech_constraints::SPEECH_CODE_MAX_REPAIR_ATTEMPTS as u64);
    let output_surface_mode = match args
        .get("output_surface_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("raw_code_only")
    {
        "fenced_transport" => speech_constraints::OutputSurfaceMode::FencedTransport,
        _ => speech_constraints::OutputSurfaceMode::RawCodeOnly,
    };
    let output_path_arg = args
        .get("output_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let vcs_agent_id = args.get("vcs_agent_id").and_then(|v| v.as_u64());

    if prompt.is_empty() {
        return ToolResult::<String>::err_with_remediation(
            "Missing 'prompt' parameter",
            REM_GEN_PROMPT,
        )
        .to_json();
    }

    let mut current_prompt = prompt.to_string();
    let mut retry_count = 0u64;
    let mut prev_error_sig: Option<u64> = None;
    let started = Instant::now();
    let mut first_valid_ms: Option<u128> = None;
    let mut surface_contract_failures: u64 = 0;
    let repair_stalled = false;
    let mut routing_recorded = false;

    let grammar_addon = speech_constraints::grammar_artifact_prompt_addon(&state.repository.root);
    let decode_policy = speech_constraints::ConstrainedDecodePolicy::from_env();

    loop {
        let (model, system_prompt, free_only, pref) =
            if decode_policy == speech_constraints::ConstrainedDecodePolicy::Rigid {
                let hint_stub = speech_constraints::TypeHintStub;
                let output_surface_instruction = match output_surface_mode {
                    speech_constraints::OutputSurfaceMode::RawCodeOnly => {
                        "- Return ONLY raw .vox code (no markdown fences, no prose).\n"
                    }
                    speech_constraints::OutputSurfaceMode::FencedTransport => {
                        "- Wrap output in a ```vox fenced code block.\n"
                    }
                };
                let sys = format!(
                    "You are an expert compiler engineer. Generate VALD .vox code.\n\n\
                 Rules:\n\
                 - Only output the code, no explanation.\n\
                 {}\n\
                 {}\n\
                 {}\n\
                 {}\n",
                    output_surface_instruction,
                    grammar_addon,
                    hint_stub.system_prompt_addon(),
                    crate::chat_tools::ANTI_LAZINESS_RIDER
                );

                let resolution_template = crate::llm_bridge::McpChatModelResolution {
                    complexity: 2,
                    ..Default::default()
                };

                let p: Option<String> = match crate::sync_poison::poison_rw_read(
                    state.mcp_chat_model_override.read(),
                    "mcp_chat_model_override",
                ) {
                    Ok(g) => (*g).clone(),
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(
                            e.to_string(),
                            REM_MCP_MODEL_LOCK,
                        )
                        .to_json();
                    }
                };
                let (m, f) = match crate::chat_model_resolve::resolve_chat_llm_model(
                    state,
                    &current_prompt,
                    resolution_template.clone(),
                    session_id.as_deref(),
                )
                .await
                {
                    Ok(pair) => pair,
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(
                            format!("No model: {e}"),
                            REM_MCP_MODEL_RESOLVE,
                        )
                        .to_json();
                    }
                };
                (m, sys, f, p)
            } else {
                // Fallback for None/Soft policy
                let sys = format!(
                    "You are an expert compiler engineer. Generate VALD .vox code.\n\n\
                 Rules:\n\
                 - Only output the code, no explanation.\n\
                 {}\n",
                    crate::chat_tools::ANTI_LAZINESS_RIDER
                );
                let resolution_template = crate::llm_bridge::McpChatModelResolution {
                    complexity: 2,
                    ..Default::default()
                };
                let p: Option<String> = match crate::sync_poison::poison_rw_read(
                    state.mcp_chat_model_override.read(),
                    "mcp_chat_model_override",
                ) {
                    Ok(g) => (*g).clone(),
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(
                            e.to_string(),
                            REM_MCP_MODEL_LOCK,
                        )
                        .to_json();
                    }
                };
                let (m, f) = match crate::chat_model_resolve::resolve_chat_llm_model(
                    state,
                    &current_prompt,
                    resolution_template.clone(),
                    session_id.as_deref(),
                )
                .await
                {
                    Ok(pair) => pair,
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(
                            format!("No model: {e}"),
                            REM_MCP_MODEL_RESOLVE,
                        )
                        .to_json();
                    }
                };
                (m, sys, f, p)
            };

        if !routing_recorded {
            if let Some(db) = &state.db {
                let reason = serde_json::json!({
                    "tool": "vox_generate_code",
                    "free_only": free_only,
                });
                let reason_s = reason.to_string();
                let _ = db
                    .record_routing_decision(
                        Some(journey_id.as_str()),
                        state.repository.repository_id.as_str(),
                        session_id.as_deref(),
                        "vox_generate_code",
                        Some(model.id.as_str()),
                        Some(reason_s.as_str()),
                    )
                    .await;
            }
            routing_recorded = true;
        }

        let routing = crate::llm_bridge::McpInferRouting {
            user_prompt: &current_prompt,
            sticky_model_pref: pref.as_deref(),
            resolution_template: crate::llm_bridge::McpChatModelResolution {
                complexity: 2,
                ..Default::default()
            },
            free_only,
            allow_cloud_ollama_fallback: true,
            user_id: session_id.as_deref(),
        };

        let (mut completion, _, _) = match crate::llm_bridge::mcp_infer_completion(
            state,
            model,
            "vox_generate_code",
            &system_prompt,
            &routing,
            2048,
            0.1,
            None,
            None,
            false,
            None,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::<String>::err_with_remediation(
                    format!("LLM error: {e}"),
                    REM_LLM_COMPLETION,
                )
                .to_json();
            }
        };

        let normalized = vox_compiler::generated_vox::normalize_generated_vox(
            &completion,
            match output_surface_mode {
                speech_constraints::OutputSurfaceMode::RawCodeOnly => {
                    vox_compiler::generated_vox::OutputSurfaceMode::RawCodeOnly
                }
                speech_constraints::OutputSurfaceMode::FencedTransport => {
                    vox_compiler::generated_vox::OutputSurfaceMode::FencedTransport
                }
            },
        );
        completion = normalized.normalized;
        if decode_policy.is_enabled() {
            if !normalized.surface_contract_ok
                || !decode_policy.surface_contract_ok(&completion, output_surface_mode)
            {
                surface_contract_failures += 1;
                retry_count += 1;
                if retry_count > max_retries {
                    let kpi = runtime_generation_kpi(
                        false,
                        validate_flag,
                        None,
                        None,
                        retry_count,
                        repair_stalled,
                        first_valid_ms,
                        completion.split_whitespace().count() as u64,
                        surface_contract_failures,
                        Some("surface_contract"),
                    );
                    return ToolResult::<String>::err_with_remediation_meta(
                        "constrained-decode policy rejected non-code surface repeatedly"
                            .to_string(),
                        REM_CODEGEN_REPAIR,
                        serde_json::json!({
                            "repair": {
                                "attempts": retry_count,
                                "failure_category": "surface_contract",
                                "decode_policy": "grammar_surface_guard"
                            },
                            "runtime_generation_kpi": kpi
                        }),
                    )
                    .to_json();
                }
                current_prompt.push_str(
                    "\n\nYour previous output violated output-surface constraints. Regenerate using only the required output mode.",
                );
                continue;
            }
        }

        if !validate_flag {
            let kpi = runtime_generation_kpi(
                true,
                false,
                None,
                None,
                retry_count + 1,
                repair_stalled,
                Some(started.elapsed().as_millis()),
                completion.split_whitespace().count() as u64,
                surface_contract_failures,
                None,
            );
            let mut meta = serde_json::json!({ "runtime_generation_kpi": kpi });
            if let Some(ref op) = output_path_arg {
                match merge_meta_after_codegen_write(
                    state,
                    meta.clone(),
                    op,
                    &completion,
                    vcs_agent_id,
                )
                .await
                {
                    Ok(m) => meta = m,
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(e, REM_GEN_OUTPUT_PATH)
                            .to_json();
                    }
                }
            }
            return ToolResult::ok_with_meta(completion, meta).to_json();
        }

        let validation = vox_compiler::generated_vox::validate_generated_vox(&completion, true);
        let errors: Vec<_> = validation.errors.iter().collect();

        if errors.is_empty() {
            if first_valid_ms.is_none() {
                first_valid_ms = Some(started.elapsed().as_millis());
            }
            let canonicalized = validation.canonicalized;
            let canonical_pass = canonicalized.is_some();
            let canonical = canonicalized.unwrap_or_else(|| completion.clone());
            let kpi = runtime_generation_kpi(
                true,
                true,
                Some(true),
                Some(canonical_pass),
                retry_count + 1,
                repair_stalled,
                first_valid_ms,
                canonical.split_whitespace().count() as u64,
                surface_contract_failures,
                None,
            );
            let mut meta = serde_json::json!({ "runtime_generation_kpi": kpi });
            if let Some(ref op) = output_path_arg {
                match merge_meta_after_codegen_write(
                    state,
                    meta.clone(),
                    op,
                    &canonical,
                    vcs_agent_id,
                )
                .await
                {
                    Ok(m) => meta = m,
                    Err(e) => {
                        return ToolResult::<String>::err_with_remediation(e, REM_GEN_OUTPUT_PATH)
                            .to_json();
                    }
                }
            }
            return ToolResult::ok_with_meta(canonical, meta).to_json();
        }

        let snapshot: Vec<serde_json::Value> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "severity": "error",
                    "message": e.message,
                    "code": e.code,
                    "category": e.category,
                })
            })
            .collect();
        let mut rows: Vec<(String, String, String)> = errors
            .iter()
            .map(|e| {
                (
                    e.category.to_string(),
                    e.message.split_whitespace().collect::<Vec<_>>().join(" "),
                    e.code.clone().unwrap_or_default(),
                )
            })
            .collect();
        rows.sort();
        let mut hasher = DefaultHasher::new();
        for row in rows {
            row.hash(&mut hasher);
        }
        let sig = hasher.finish();
        if prev_error_sig == Some(sig) {
            let kpi = runtime_generation_kpi(
                false,
                true,
                Some(false),
                Some(false),
                retry_count,
                true,
                first_valid_ms,
                completion.split_whitespace().count() as u64,
                surface_contract_failures,
                Some("repair_stall"),
            );
            return ToolResult::<String>::err_with_remediation_meta(
                format!(
                    "repair loop stalled: diagnostics unchanged after retry (signature={sig:#x})"
                ),
                REM_CODEGEN_STALL,
                serde_json::json!({
                    "diagnostics_snapshot": snapshot,
                    "repair": { "attempts": retry_count, "stalled": true },
                    "runtime_generation_kpi": kpi
                }),
            )
            .to_json();
        }
        prev_error_sig = Some(sig);

        retry_count += 1;
        if retry_count > max_retries {
            let err_msgs: Vec<_> = errors.iter().map(|e| &e.message).collect();
            let kpi = runtime_generation_kpi(
                false,
                true,
                Some(false),
                Some(false),
                retry_count,
                false,
                first_valid_ms,
                completion.split_whitespace().count() as u64,
                surface_contract_failures,
                Some("semantic"),
            );

            if let Some(db) = state.db.as_ref() {
                let snapshot_for_json: Vec<serde_json::Value> = validation
                    .errors
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "category": e.category,
                            "code": e.code,
                            "message": e.message,
                        })
                    })
                    .collect();
                let errors_json = serde_json::to_string(&snapshot_for_json).unwrap_or_default();
                let _ = vox_corpus::tool_workflow_corpus::auto_ingest_negative_vox(
                    &completion,
                    &errors_json,
                    db,
                )
                .await;
            }

            return ToolResult::<String>::err_with_remediation_meta(
                format!(
                    "Failed to generate valid code after {} retries. Errors: {:?}",
                    max_retries, err_msgs
                ),
                REM_CODEGEN_REPAIR,
                serde_json::json!({
                    "diagnostics_snapshot": snapshot,
                    "repair": {
                        "attempts": retry_count,
                        "stalled": false,
                        "failure_category": "semantic"
                    },
                    "runtime_generation_kpi": kpi
                }),
            )
            .to_json();
        }

        // Add diagnostics to prompt for retry
        let mut feedback = String::from(
            "\n\nThe previous generation had these errors. Fix them and re-generate ONLY the corrected .vox code:\n",
        );
        for (i, err) in errors.iter().enumerate() {
            feedback.push_str(&format!("{}. [{}] {}\n", i + 1, err.category, err.message));
        }
        if let Ok(json) =
            serde_json::to_string_pretty(&serde_json::json!({ "diagnostics_snapshot": &snapshot }))
        {
            feedback.push_str("\nStructured diagnostics (JSON):\n");
            feedback.push_str(&json);
            feedback.push('\n');
        }
        current_prompt.push_str(&feedback);
    }
}

/// Apply a structured edit with AST/HIR validation and an auto-repair loop.
pub async fn apply_structured_edit(state: &ServerState, args: serde_json::Value) -> String {
    let started = std::time::Instant::now();
    let file_path = match args.get("file_path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return ToolResult::<String>::err("Missing file_path").to_json(),
    };
    let target_content = match args.get("target_content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return ToolResult::<String>::err("Missing target_content").to_json(),
    };
    let mut replacement_code = match args.get("replacement_code").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return ToolResult::<String>::err("Missing replacement_code").to_json(),
    };
    let session_id = args.get("session_id").and_then(|v| v.as_str());

    let abs_path = state.repository.root.join(file_path);
    let original_text = match tokio::fs::read_to_string(&abs_path).await {
        Ok(t) => t,
        Err(e) => {
            return ToolResult::<String>::err(format!("Failed to read file: {}", e)).to_json();
        }
    };

    if !original_text.contains(target_content) {
        return ToolResult::<String>::err("target_content not found in file").to_json();
    }

    let extension = std::path::Path::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let max_retries = 3;
    let mut retry_count = 0;

    loop {
        let new_text = original_text.replace(target_content, &replacement_code);

        let mut error_feedback = String::new();

        if extension == "vox" {
            let validation = vox_compiler::generated_vox::validate_generated_vox(&new_text, true);
            let errors: Vec<_> = validation.errors.iter().collect();
            if !errors.is_empty() {
                for err in &errors {
                    error_feedback.push_str(&format!("- [{}] {}\n", err.category, err.message));
                }
            }
        } else if extension == "rs" {
            // Shadow Apply: write, cargo check, revert
            let _ = tokio::fs::write(&abs_path, new_text.as_bytes()).await;
            let mut cmd = tokio::process::Command::new("cargo");
            cmd.current_dir(&state.repository.root);
            cmd.arg("check").arg("--message-format=short");
            if state.repository.capabilities.cargo_workspace {
                cmd.arg("--workspace");
            }
            if let Ok(output) = cmd.output().await {
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error_feedback = stderr.to_string();
                }
            } else {
                error_feedback = "Failed to run cargo check".to_string();
            }
            // Revert shadow apply if there was an error
            if !error_feedback.is_empty() {
                let _ = tokio::fs::write(&abs_path, original_text.as_bytes()).await;
            }
        }

        if error_feedback.is_empty() {
            // Write validated code (already written if `.rs` and passed, but we'll ensure it here)
            if let Err(e) = tokio::fs::write(&abs_path, new_text.as_bytes()).await {
                return ToolResult::<String>::err(format!("Failed to write file: {}", e)).to_json();
            }
            let first_valid_ms = started.elapsed().as_millis();
            let kpi = runtime_generation_kpi(
                true,
                true,
                Some(true),
                Some(true),
                retry_count + 1,
                false,
                Some(first_valid_ms),
                replacement_code.split_whitespace().count() as u64,
                0,
                None,
            );
            tracing::info!(
                target: "vox_mcp_speech",
                time_to_first_valid_ms = first_valid_ms as u64,
                repair_attempts = retry_count + 1,
                output_tokens = replacement_code.split_whitespace().count() as u64,
                "apply_structured_edit runtime_kpi"
            );
            let meta = serde_json::json!({ "runtime_generation_kpi": kpi });
            return ToolResult::ok_with_meta(
                "Edit applied successfully (structural validation passed).".to_string(),
                meta,
            )
            .to_json();
        }

        retry_count += 1;
        if retry_count > max_retries {
            let kpi = runtime_generation_kpi(
                false,
                true,
                Some(false),
                Some(false),
                retry_count,
                false,
                None,
                replacement_code.split_whitespace().count() as u64,
                0,
                Some("semantic"),
            );
            let meta = serde_json::json!({
                "runtime_generation_kpi": kpi,
                "repair": {
                    "attempts": retry_count,
                    "stalled": false,
                    "failure_category": "semantic"
                }
            });
            return ToolResult::<String>::err_with_remediation_meta(
                format!(
                    "Failed to generate valid edit after {} retries. Errors: {}",
                    max_retries, error_feedback
                ),
                REM_CODEGEN_REPAIR,
                meta,
            )
            .to_json();
        }

        // Auto-repair via LLM
        let mut feedback = format!(
            "Your replacement code for {} introduced compiler errors. Fix ONLY the replacement code.\n\nErrors:\n{}",
            file_path, error_feedback
        );
        feedback.push_str("\n\nOriginal replacement code:\n```\n");
        feedback.push_str(&replacement_code);
        feedback.push_str("\n```\nProvide ONLY the fixed replacement code, no fences.");

        let resolution_template = crate::llm_bridge::McpChatModelResolution {
            complexity: 2,
            ..Default::default()
        };

        if let Ok((model, free_only)) = crate::chat_model_resolve::resolve_chat_llm_model(
            state,
            &feedback,
            resolution_template.clone(),
            session_id,
        )
        .await
        {
            let routing = crate::llm_bridge::McpInferRouting {
                user_prompt: &feedback,
                sticky_model_pref: None,
                resolution_template,
                free_only,
                allow_cloud_ollama_fallback: true,
                user_id: session_id,
            };

            if let Ok((completion, _, _)) = crate::llm_bridge::mcp_infer_completion(
                state,
                model,
                "vox_apply_structured_edit",
                "You are an expert compiler engineer. Fix the replacement code. Output ONLY raw code, no fences.",
                &routing,
                2048,
                0.2,
                None,
                None,
                false,
                None,
            ).await {
                replacement_code = vox_compiler::generated_vox::normalize_generated_vox(
                    &completion,
                    vox_compiler::generated_vox::OutputSurfaceMode::RawCodeOnly,
                ).normalized;
                continue;
            }
        }

        break;
    }

    ToolResult::<String>::err("Failed to repair edit automatically due to LLM error.").to_json()
}
