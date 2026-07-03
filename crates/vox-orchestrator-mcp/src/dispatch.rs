//! `handle_tool_call` routing for all static MCP tools.
//!
//! ## Persisted tool args (Ludus / raw `tool_call` rows)
//! After each dispatch, when Codex is attached, stored payloads use
//! [`vox_gamify::mcp_privacy::prepare_mcp_tool_args_for_storage`] for **both** Ludus-routed `mcp_tool_called` events and the
//! fallback `insert_event` path. New DB persistence for MCP args must go through the same helper + env (`VOX_LUDUS_MCP_TOOL_ARGS`).

use crate::params::ToolResult;
use crate::server_state::ServerState;
use vox_telemetry::{EditPatternEvent, ErrorSurfaceEvent, HarnessUsageEvent, TelemetryEvent};

#[cfg(feature = "heavy-browser")]
use crate::browser_tools;
#[cfg(feature = "gui-visual-review")]
use crate::visus_tools;
use crate::{
    agent_tools, benchmark_tools, chat_tools, code_validator, codex_tools, compiler_tools,
    db_tools, exec_time_tools, feedback_tools, git_tools, grammar_tools, introspection_tools,
    openclaw_tools, persistence_tools, populi_tools, project_init_tools, questioning_tools,
    rag_tools, repo_catalog_tools, repo_index, secrets_tools, task_tools, toestub_tools,
    tool_aliases, training_tools, trust_tools, vcs_tools,
};
#[cfg(feature = "news-publish")]
use crate::{news_tools, scientia_tools};

#[cfg(feature = "oratio-rerank")]
use crate::{oratio_tools, speech_pipeline_tools};

/// Dispatch `name` to the matching submodule handler and record skill telemetry if DB is available.
///
/// Equivalent to calling [`handle_tool_call_with_mode`] with `permission_mode: None`
/// (i.e. today's baseline `ask` / always-park behavior for dangerous tools).
/// Kept as the primary entry point since most callers (stdio MCP server, HTTP
/// gateway, tests) have no `PermissionMode` to thread through — only the
/// daemon's authenticated `orch.tool_call` path (T0.2/T0.3) carries one.
pub async fn handle_tool_call(
    state: &ServerState,
    name: &str,
    args: serde_json::Value,
) -> Result<String, anyhow::Error> {
    handle_tool_call_with_mode(state, name, args, None).await
}

/// Same as [`handle_tool_call`], but accepts an explicit `permission_mode`
/// wire string (T0.3 — `DispatchRequest::permission_mode`, mirrored via
/// `OrchDaemonClient`). `None` (or any unrecognized value) resolves to
/// [`crate::permission_modes::PermissionMode::Ask`] — the fail-safe default
/// that matches pre-T0.3 always-park behavior. `permission_mode` must NEVER
/// be sourced from `args` (tool-call params the LLM agent composes) — only
/// from this explicit parameter, which callers populate from the
/// authenticated transport layer, never from caller-supplied JSON.
pub async fn handle_tool_call_with_mode(
    state: &ServerState,
    name: &str,
    args: serde_json::Value,
    permission_mode: Option<&str>,
) -> Result<String, anyhow::Error> {
    let start_time = std::time::Instant::now();
    let name_canonical = tool_aliases::canonical_tool_name(name);

    // Check if the agent ID or session ID is included in meta arguments
    let agent_id = args.get("agent_id").and_then(|v| v.as_str());
    let session_id = args.get("session_id").and_then(|v| v.as_str());
    let trace_for_telemetry = args
        .get("trace_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            args.get("correlation_id")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
        });
    // T1.5: best-effort correlation fallback for the dangerous-tool approval
    // gate below. No dedicated `run_id`-shaped field is threaded from the GUI
    // through `DispatchRequest`/`ServerState` today (verified: `invoke_mcp_tool`
    // in crates/vox-gui/src/commands/mcp.rs only forwards `permission_mode` as a
    // top-level field; `trace_id`/`correlation_id` are caller-composed `args`
    // that agent tool-calls rarely set). The one identifier that reliably IS
    // present on tool calls issued while executing an orchestrator task is the
    // numeric `task_id` (see `ctx.task_id` below, and
    // `vox_telemetry::TaskRootSummaryEvent::task_id`, which is the same value
    // `submit_orchestrator_task` returns to the GUI). Using it as the
    // `ApprovalRequested`/`ApprovalResolved` `run_id` when no explicit
    // trace/correlation id was supplied lets `finish_gui_run` join a run's
    // approval by `task_id` without adding new top-level plumbing — see
    // docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md T1.5.
    let run_id_for_approval = trace_for_telemetry.clone().or_else(|| {
        args.get("task_id")
            .and_then(|v| v.as_u64())
            .map(|t| t.to_string())
    });

    // Check Budget limits for explicit Tool interception (Agent Self-Correction)
    let b_signal = {
        let aid = agent_id.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let bm = state.orchestrator.budget_manager_handle();
        vox_orchestrator::sync_lock::rw_read(&*bm)
            .agent_budget_signal(vox_orchestrator::types::AgentId(aid))
    };

    if matches!(
        b_signal,
        vox_orchestrator::budget::BudgetSignal::CostExceeded { .. }
            | vox_orchestrator::budget::BudgetSignal::Critical { .. }
    ) {
        return Ok(crate::params::ToolResult::<()>::err("SYSTEM_INTERVENTION: You have exceeded your global task budget. Proceed to finalize and abort immediately.").to_json_compact());
    }

    // Unenforced LLM "Laziness" Ingestion Gate
    if matches!(
        name_canonical,
        "vox_write_file"
            | "vox_patch_file"
            | "vox_inline_edit_file"
            | "vox_multi_replace"
            | "vox_multi_replace_file"
    ) {
        let args_str = args.to_string();
        if args_str.contains("todo!()")
            || args_str.contains("unimplemented!()")
            || args_str.contains("// TODO")
        {
            return Ok(crate::params::ToolResult::<()>::err("LAZY_GENERATION_DETECTED: The system intercepted a TOESTUB pattern (e.g. todo!(), unimplemented!(), or // TODO) in your code output. You must emit the complete, fully-implemented code. Re-run your action with the actual logic.").to_json_compact());
        }
    }

    if let Some(rejection) = crate::scope_guard::check_scope(state, name_canonical, agent_id, &args)
    {
        return Ok(crate::params::ToolResult::<()>::err(rejection).to_json_compact());
    }

    if let Some(rejection) = crate::lock_guard::check_lock(state, name_canonical, &args) {
        return Ok(crate::params::ToolResult::<()>::err(rejection).to_json_compact());
    }

    if let Some(rejection) = crate::skill_permissions::check_skill_tool_permission(
        &state.skill_registry,
        state.active_skill_id.read().as_deref(),
        name_canonical,
    ) {
        return Ok(crate::params::ToolResult::<()>::err(rejection).to_json_compact());
    }

    if state.orchestrator_config.agentos_guardrail_kernel_enabled {
        if let Err(detail) =
            vox_orchestrator::agentos::guardrail_kernel::evaluate_mcp_tool_preflight(
                name_canonical,
                &args,
            )
        {
            crate::agentos_telemetry::record_guardrail_deny_best_effort(
                state.db.as_ref(),
                state.repository.repository_id.as_str(),
                &detail,
            )
            .await;
            return Ok(crate::params::ToolResult::<()>::err(detail.reason).to_json_compact());
        }
    }

    // Trust-Tier RBAC for dangerous operations. T0.3: gate membership is now
    // registry-driven (crate::permission_modes::RISK_CLASSES, mirroring
    // contracts/orchestration/permission-modes.v1.yaml) instead of a hardcoded
    // tool-name allowlist. A tool absent from the registry is `unknown` and
    // skips this gate entirely — same shape as the pre-T0.3 hardcoded list
    // (this is an allowlist-of-dangerous-tools, not a denylist; see the
    // tool_aliases hardening note in the contract file and in
    // crate::permission_modes for the documented scope of that exposure).
    if crate::permission_modes::is_gated_tool(name_canonical) {
        // T0.3 precedence tiers 2 + 3: a caller-selected PermissionMode
        // (never sourced from `args` — see `handle_tool_call_with_mode`'s
        // doc comment) or a persisted per-repo allowlist entry may
        // auto-approve this call, skipping the park-and-await below
        // entirely. Falls through to the unconditional HITL park otherwise
        // (today's baseline `ask`-mode behavior, byte-for-byte unchanged).
        let mode = crate::permission_modes::PermissionMode::from_wire(permission_mode);
        // `mode_auto_approves` already returns `false` unconditionally for
        // any tool with `always_requires_approval: true` (e.g.
        // vox_add_approval_allowlist_entry — see the T0.3 follow-up review
        // finding), so no separate check is needed here for tier 2.
        let mode_auto_approved = crate::permission_modes::mode_auto_approves(mode, name_canonical);
        let allowlisted = if mode_auto_approved {
            false // short-circuit: no need to hit the DB if the mode already approved
        } else if !crate::permission_modes::allowlist_eligible(name_canonical) {
            // Tier 3 is not even consulted for an `always_requires_approval`
            // tool — it must always park regardless of what's persisted.
            false
        } else {
            crate::approval_allowlist::is_allowlisted(
                state.repository.repository_id.as_str(),
                name_canonical,
            )
            .await
        };

        if !mode_auto_approved && !allowlisted {
            // B3 HITL: unconditionally register an interactive approval and await the
            // human decision (resolved in-process via the `vox_resolve_approval` tool).
            // There is NO arg-based fast path here — a tool-call argument must never be
            // able to skip human approval for a dangerous operation, since the LLM
            // agent itself composes the tool-call JSON. See T0.1 in
            // docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let summary = {
                let a = args.to_string();
                let a: String = a.chars().take(200).collect();
                format!("{name_canonical} {a}")
            };
            let (approval_id, rx) = state.pending_approvals.register(
                name_canonical.to_string(),
                summary.clone(),
                now_ms,
            );
            // Durable audit trail (best-effort): record the request, then its outcome.
            if let Some(db) = state.db.as_ref() {
                let _ = db
                    .hitl_approval_record(&approval_id, name_canonical, &summary, now_ms as i64)
                    .await;
            }
            // T1.1: op-log is the dispatch-lifecycle SSOT (durable independently of
            // hitl_approvals, which stays as a derived/joined table — see the
            // reconciliation table in vox-axis-harness-reliability-spec-plan-2026-07-02.md
            // section 2). `record_operation` acquires the std RwLock write guard
            // synchronously, releases it before its own internal `.await`, and is
            // therefore safe to call directly here (same pattern as vcs_ops.rs's
            // existing `record` caller).
            state
                .orchestrator
                .record_operation(
                    vox_orchestrator::AgentId(0),
                    vox_orchestrator::oplog::OperationKind::ApprovalRequested {
                        approval_id: approval_id.clone(),
                        tool: name_canonical.to_string(),
                        run_id: run_id_for_approval.clone(),
                    },
                    format!("Approval requested for {name_canonical}"),
                    None,
                    None,
                    None,
                    None,
                )
                .await;
            const APPROVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
            let outcome = match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
                Ok(Ok(o)) => o,
                Ok(Err(_)) => vox_orchestrator::ApprovalOutcome::Rejected, // resolver dropped
                Err(_) => {
                    state.pending_approvals.cancel(&approval_id);
                    vox_orchestrator::ApprovalOutcome::TimedOut
                }
            };
            let resolved_status = format!("{outcome:?}").to_lowercase();
            if let Some(db) = state.db.as_ref() {
                let resolved_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let _ = db
                    .hitl_approval_resolve(&approval_id, &resolved_status, resolved_ms)
                    .await;
            }
            // T1.1: durable ApprovalResolved alongside the hitl_approvals write above.
            state
                .orchestrator
                .record_operation(
                    vox_orchestrator::AgentId(0),
                    vox_orchestrator::oplog::OperationKind::ApprovalResolved {
                        approval_id: approval_id.clone(),
                        outcome: resolved_status,
                        resolver: None,
                    },
                    format!("Approval resolved for {name_canonical}"),
                    None,
                    None,
                    None,
                    None,
                )
                .await;
            if !matches!(
                outcome,
                vox_orchestrator::ApprovalOutcome::Approved
                    | vox_orchestrator::ApprovalOutcome::Modified
            ) {
                return Ok(crate::params::ToolResult::<()>::err(format!(
                    "Operation '{name_canonical}' was not approved (outcome: {outcome:?})."
                ))
                .to_json_compact());
            }
            // Approved / Modified: fall through and execute the tool.
        }
        // mode_auto_approved || allowlisted: skip the park entirely, fall
        // through and execute the tool immediately.
    }

    // Build a TraceContext from the incoming call metadata so all async code reachable
    // from this tool dispatch (LLM calls, sub-dispatches) can read it via current_trace_ctx().
    let trace_ctx = {
        use uuid::Uuid;
        use vox_telemetry::TraceContext;
        let mut ctx = TraceContext::default();
        if let Some(tid_str) = trace_for_telemetry.as_deref() {
            if let Ok(parsed) = Uuid::parse_str(tid_str) {
                ctx.trace_id = parsed;
            }
        }
        ctx.task_id = args.get("task_id").and_then(|v| v.as_u64());
        ctx.parent_task_id = args.get("parent_task_id").and_then(|v| v.as_u64());
        ctx.caller_agent_id = agent_id.map(ToString::to_string);
        ctx.span_depth = args
            .get("span_depth")
            .and_then(|v| v.as_u64())
            .map(|d| d.min(u16::MAX as u64) as u16)
            .unwrap_or(0);
        ctx
    };

    // Phase D: anchor wall-clock start time for this task in the aggregator.
    // record_task_started is idempotent (no-op if already set), so calling it on
    // every tool dispatch safely records the first-call instant without overwriting.
    if let Some(task_id) = trace_ctx.task_id {
        vox_telemetry::record_task_started(task_id);
    }

    let db_opt = state.db.as_ref().map(|db| (**db).clone());
    let te = vox_db::TimedExecution::new(
        format!("mcp:{}", name_canonical),
        &state.repository.repository_id,
        None,
        db_opt,
    )
    .with_costs(None, None, None);

    let aci_envelope = state.orchestrator_config.agentos_aci_envelope_enabled;
    let checkpoint_hints = state.orchestrator_config.agentos_checkpoint_hints_enabled;

    // T4.3: outer execution timeout around the actual tool dispatch, sourced
    // per-tool from `dispatch_timeout::timeout_for` (agy delegation tools get
    // a much larger exception; everything else gets the global default). This
    // is independent of and does NOT replace the 300s HITL approval-wait
    // above — that bounds waiting for a human decision before execution ever
    // starts; this bounds the execution itself, for every tool, gated or not.
    // `vox_db::TimedExecution` (the `te` used below) only measures duration
    // for telemetry and never bounded it, so a hung tool implementation
    // previously blocked this handler (and the MCP connection) indefinitely.
    let call_timeout = crate::dispatch_timeout::timeout_for(name_canonical);
    let result = te
        .run(|| {
            let args = args.clone();
            async move {
                match tokio::time::timeout(
                    call_timeout,
                    vox_telemetry::TRACE_CTX.scope(trace_ctx, async move {
                        handle_tool_call_inner(state, name_canonical, args).await
                    }),
                )
                .await
                {
                    Ok(inner) => inner,
                    Err(_elapsed) => {
                        tracing::warn!(
                            tool = name_canonical,
                            timeout_secs = call_timeout.as_secs(),
                            "tool execution exceeded its dispatch timeout"
                        );
                        Ok(crate::params::ToolResult::<()>::err(format!(
                            "Tool '{name_canonical}' exceeded its execution timeout ({}s) and was aborted. \
                             This is a dispatch-level guard independent of any approval wait; the tool call \
                             itself took too long to complete.",
                            call_timeout.as_secs()
                        ))
                        .to_json_compact())
                    }
                }
            }
        })
        .await;

    // AgentOS: fold MCP mutation_kind into live orchestrator policy ledger (D5 overlay input).
    {
        let aid = agent_id.and_then(|s| s.parse::<u64>().ok());
        state
            .orchestrator
            .record_agentos_mcp_tool(aid, name_canonical);
    }

    let result = result.map(|payload| {
        if !aci_envelope {
            return payload;
        }
        match crate::aci::attach_aci_envelope(
            name_canonical,
            &payload,
            checkpoint_hints,
            Some(&args),
        ) {
            Ok(wrapped) => wrapped,
            Err(e) => {
                tracing::warn!(tool = name_canonical, error = %e, "aci envelope attach failed; returning raw payload");
                payload
            }
        }
    });

    // Verification-driven loop: after a successful file-mutating tool, auto-run
    // `vox check` on the touched `.vox` file and surface any error diagnostics back
    // inside the tool result, so the agent self-corrects on its next turn without
    // having to remember to validate. No-op for non-mutating tools, non-`.vox` paths,
    // clean files, or when disabled via `VOX_VERIFY_ON_WRITE`. See `post_verification`.
    let result = match result {
        Ok(payload) => {
            Ok(
                crate::post_verification::verify_and_attach(state, name_canonical, &args, payload)
                    .await,
            )
        }
        Err(e) => Err(e),
    };

    let duration_ms = start_time.elapsed().as_millis() as i64;

    // Operation capture (sub-project 1): best-effort, redacted, fire-and-forget.
    // Runs only for executed tools (guard rejections returned earlier).
    crate::operation_capture::spawn_capture(
        state.db.clone(),
        state.orchestrator_config.operations_capture_enabled,
        name_canonical.to_string(),
        args.clone(),
        match &result {
            Ok(s) => s.clone(),
            Err(e) => e.to_string(),
        },
        session_id.map(|s| s.to_string()),
        agent_id.map(|s| s.to_string()),
        duration_ms,
        result.is_err(),
    );

    // Track E — emit structured telemetry for every MCP tool call.
    {
        let tool_call_kind = tool_call_kind_for(name_canonical);
        let mode = if agent_id.is_some() {
            "agent"
        } else {
            "interactive"
        };
        vox_telemetry::record_event!(&TelemetryEvent::HarnessUsage(HarnessUsageEvent {
            tool_call_kind: tool_call_kind.to_string(),
            mode: mode.to_string(),
        }));

        // edit_pattern — only on successful file mutations.
        if result.is_ok() && is_file_mutation(name_canonical) {
            let op_type = file_op_type(name_canonical);
            let file_kind = args
                .get("path")
                .and_then(|v| v.as_str())
                .map(file_kind_from_path)
                .unwrap_or("unknown");
            let content_len = args
                .get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.len())
                .or_else(|| {
                    args.get("new_content")
                        .and_then(|v| v.as_str())
                        .map(|s| s.len())
                })
                .unwrap_or(0);
            let size_bucket = content_size_bucket(content_len);
            vox_telemetry::record_event!(&TelemetryEvent::EditPattern(EditPatternEvent {
                op_type: op_type.to_string(),
                file_kind: file_kind.to_string(),
                size_bucket: size_bucket.to_string(),
            }));
        }

        // error_surface — only on failures.
        if let Err(ref e) = result {
            let error_class = error_class_from_err(e);
            let subsystem = subsystem_from_tool(name_canonical);
            vox_telemetry::record_event!(&TelemetryEvent::ErrorSurface(ErrorSurfaceEvent {
                error_class: error_class.to_string(),
                subsystem: subsystem.to_string(),
                recoverable: false,
            }));
        }
    }

    if let Some(ref tid) = trace_for_telemetry {
        tracing::info!(
            target: "vox_mcp::trace",
            trace_id = %tid,
            tool = name_canonical,
            duration_ms,
            success = result.is_ok(),
            "mcp_tool_call"
        );
    }

    // Ludus: canonical reward path when enabled; raw telemetry when gamification is off.
    if let Some(db) = &state.db {
        let aid = agent_id.and_then(|s| s.parse::<u64>().ok()).unwrap_or(0u64);
        let args_stored = vox_gamify::mcp_privacy::prepare_mcp_tool_args_for_storage(&args);
        let mut route_ev = serde_json::json!({
            "type": "mcp_tool_called",
            "agent_id": aid,
            "tool": name_canonical,
            "args": args_stored,
            "duration_ms": duration_ms,
            "success": result.is_ok(),
            "repository_id": state.repository.repository_id,
            "mutation_kind": vox_orchestrator::agentos::mutation_classifier::mutation_kind_for_tool(name_canonical),
        });
        if let Some(sid) = session_id {
            route_ev["session_id"] = serde_json::Value::String(sid.to_string());
        }
        if let Some(ref tid) = trace_for_telemetry {
            route_ev["trace_id"] = serde_json::Value::String(tid.clone());
        }
        if vox_gamify::config_gate::is_enabled() {
            let _ = vox_gamify::event_router::route_event_auto_user(db, &route_ev).await;
        } else {
            let mut payload = serde_json::json!({
                "type": "tool_call",
                "tool": name_canonical,
                "args": args_stored,
                "duration_ms": duration_ms,
                "success": result.is_ok(),
                "repository_id": state.repository.repository_id,
                "mutation_kind": vox_orchestrator::agentos::mutation_classifier::mutation_kind_for_tool(name_canonical),
            });
            if let Some(sid) = session_id {
                payload["session_id"] = serde_json::Value::String(sid.to_string());
            }
            if let Some(ref tid) = trace_for_telemetry {
                payload["trace_id"] = serde_json::Value::String(tid.clone());
            }
            let agent_str = agent_id.unwrap_or("0");
            let _ = vox_gamify::db::insert_event(
                db,
                agent_str,
                "tool_call",
                Some(&payload.to_string()),
            )
            .await;
        }
    }

    result
}
async fn handle_tool_call_inner(
    state: &ServerState,
    name: &str,
    args: serde_json::Value,
) -> Result<String, anyhow::Error> {
    {
        let ws = state.workspace_mcp.read();
        if ws.tool_by_name(name).is_some() {
            return match crate::workspace_mcp::dispatch_workspace_tool(&ws, name, &args) {
                Ok(json) => Ok(json),
                Err(e) => Ok(ToolResult::<()>::err(e).to_json()),
            };
        }
    }

    // T4.3 RED-test doubles: never compiled into a non-test build. Lets the
    // dispatch-timeout tests exercise the real `handle_tool_call` /
    // `handle_tool_call_with_mode` path (approval gate, TimedExecution,
    // telemetry, the new outer `tokio::time::timeout`) end-to-end without
    // depending on any real tool's I/O or subprocess behavior.
    #[cfg(test)]
    match name {
        "vox_test_hang_forever" => {
            let sleep_ms = args
                .get("sleep_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(u64::MAX);
            tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
            return Ok(ToolResult::ok("woke up (should not happen under test timeout)").to_json());
        }
        "vox_test_agy_like_long_running" => {
            let sleep_ms = args.get("sleep_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
            return Ok(ToolResult::ok("agy-like delegation completed").to_json());
        }
        _ => {}
    }

    match name {
        "vox_visual_rag_query" => {
            Ok(rag_tools::visual_rag_query(state, serde_json::from_value(args)?).await)
        }
        "vox_submit_task" => {
            Ok(task_tools::submit_task(state, serde_json::from_value(args)?).await)
        }
        "vox_task_status" => {
            Ok(task_tools::task_status(state, serde_json::from_value(args)?).await)
        }
        "vox_test_decision" => {
            Ok(task_tools::test_decision(state, serde_json::from_value(args)?).await)
        }
        "vox_tool_search" => Ok(crate::tool_search::vox_tool_search(serde_json::from_value(
            args,
        )?)),
        // B3 HITL: list / resolve approvals awaiting a human decision (the
        // dangerous-tool gate below parks on these).
        "vox_pending_approvals" => Ok(crate::params::ToolResult::ok(serde_json::json!({
            "approvals": state.pending_approvals.list(),
        }))
        .to_json_compact()),
        "vox_resolve_approval" => {
            let approval_id = args
                .get("approval_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let decision = args
                .get("outcome")
                .or_else(|| args.get("decision"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let outcome = crate::pending_approvals::outcome_from_decision(decision);
            let resolved = state.pending_approvals.resolve(approval_id, outcome);
            Ok(crate::params::ToolResult::ok(serde_json::json!({
                "resolved": resolved,
                "approval_id": approval_id,
                "outcome": format!("{outcome:?}"),
            }))
            .to_json_compact())
        }
        // T0.3 Part D: persist a "always allow this tool in this repo"
        // allowlist entry (tier 3 of the dangerous-tool gate's precedence
        // order — see contracts/orchestration/permission-modes.v1.yaml). The
        // GUI's ApprovalsView calls this alongside resolving the current
        // approval when the "always allow" checkbox is set. `repo_id`
        // defaults to the current server's own repository when omitted, but
        // callers should pass it explicitly to avoid ambiguity.
        "vox_add_approval_allowlist_entry" => {
            let tool = args.get("tool").and_then(|v| v.as_str()).unwrap_or("");
            let repo_id = args
                .get("repo_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(state.repository.repository_id.as_str());
            if tool.is_empty() {
                return Ok(
                    crate::params::ToolResult::<()>::err("`tool` (string) is required")
                        .to_json_compact(),
                );
            }
            match crate::approval_allowlist::add_entry(repo_id, tool).await {
                Ok(()) => Ok(crate::params::ToolResult::ok(serde_json::json!({
                    "added": true,
                    "repo_id": repo_id,
                    "tool": tool,
                }))
                .to_json_compact()),
                Err(e) => Ok(crate::params::ToolResult::<()>::err(format!(
                    "failed to persist allowlist entry: {e}"
                ))
                .to_json_compact()),
            }
        }
        // T0.3 Part D: list allowlisted tools for a repo (GUI display of
        // current allowlist state). `repo_id` defaults to this server's own
        // repository when omitted.
        "vox_list_approval_allowlist" => {
            let repo_id = args
                .get("repo_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(state.repository.repository_id.as_str());
            let tools = crate::approval_allowlist::list_for_repo(repo_id).await;
            Ok(crate::params::ToolResult::ok(serde_json::json!({
                "repo_id": repo_id,
                "tools": tools,
            }))
            .to_json_compact())
        }
        "vox_orchestrator_status" => crate::dei_tools::orchestrator_status(state).await,
        "vox_orchestrator_persistence_outbox_lifecycle" => {
            Ok(persistence_tools::persistence_outbox_lifecycle(state, args).await)
        }
        "vox_orchestrator_persistence_outbox_queue" => {
            Ok(persistence_tools::persistence_outbox_queue(state, args).await)
        }
        "vox_orchestrator_start" => Ok(crate::dei_tools::orchestrator_start(state).await),
        "vox_spawn_agent" => {
            Ok(crate::dei_tools::spawn_agent(state, serde_json::from_value(args)?).await)
        }
        "vox_retire_agent" => {
            Ok(crate::dei_tools::retire_agent(state, serde_json::from_value(args)?).await)
        }
        "vox_pause_agent" => {
            Ok(crate::dei_tools::pause_agent(state, serde_json::from_value(args)?).await)
        }
        "vox_resume_agent" => {
            Ok(crate::dei_tools::resume_agent(state, serde_json::from_value(args)?).await)
        }
        "vox_complete_task" => {
            Ok(task_tools::complete_task(state, serde_json::from_value(args)?).await)
        }
        "vox_fail_task" => Ok(task_tools::fail_task(state, serde_json::from_value(args)?).await),
        "vox_doubt_task" => Ok(task_tools::doubt_task(state, serde_json::from_value(args)?).await),
        "vox_propose_skill" => {
            Ok(feedback_tools::propose_skill(state, serde_json::from_value(args)?).await)
        }
        "vox_ask_clarification" => {
            Ok(feedback_tools::ask_clarification(state, serde_json::from_value(args)?).await)
        }
        "vox_resolve_feedback" => {
            Ok(feedback_tools::resolve_feedback(state, serde_json::from_value(args)?).await)
        }
        "vox_feedback_list" => {
            Ok(feedback_tools::feedback_list(state, serde_json::from_value(args)?).await)
        }
        "vox_check_file_owner" => Ok(crate::dei_tools::check_file_owner(
            state,
            args.get("path").and_then(|v| v.as_str()).unwrap_or("."),
        )
        .await),

        "vox_validate_file" => {
            let path_opt = args.get("path").and_then(|v| v.as_str());
            let s_id = args
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let t_id = args
                .get("task_id")
                .and_then(|v| v.as_str())
                .or_else(|| args.get("agent_id").and_then(|v| v.as_str()))
                .unwrap_or(s_id);

            // Intercept path and run observer
            if let Some(p) = path_opt {
                let resolved = crate::workspace_path::resolve_existing_path_in_repository(state, p)
                    .unwrap_or_else(|_| std::path::PathBuf::from(p));
                let report = if resolved.extension().and_then(|s| s.to_str()) == Some("rs")
                    || resolved.extension().and_then(|s| s.to_str()) == Some("vox")
                {
                    state.observer.observe_rust_file(s_id, t_id, &resolved)
                } else {
                    state.observer.observe_file(s_id, t_id, &resolved)
                };
                state.orchestrator.event_bus().emit(
                    vox_orchestrator::AgentEventKind::ObservationRecorded {
                        agent_id: vox_orchestrator::types::AgentId(t_id.parse().unwrap_or(0)),
                        task_id: vox_orchestrator::types::TaskId(t_id.parse().unwrap_or(0)),
                        file_path: resolved.clone(),
                        lsp_error_count: report.lsp_error_count,
                        parse_rate: report.parse_rate,
                        construct_coverage: report.construct_coverage,
                        recommended_action: format!("{:?}", report.recommended_action),
                    },
                );
            }

            Ok(code_validator::validate_file(state, serde_json::from_value(args)?).await)
        }
        "vox_check" => Ok(code_validator::vox_check(state, serde_json::from_value(args)?).await),
        "vox_validate_source" => {
            Ok(code_validator::validate_source(state, serde_json::from_value(args)?).await)
        }
        "vox_run_tests" => {
            Ok(compiler_tools::run_tests(state, serde_json::from_value(args)?).await)
        }
        "vox_check_workspace" => Ok(compiler_tools::check_workspace(state).await),
        "vox_test_all" => Ok(compiler_tools::test_all(state).await),
        "vox_publish_message" => {
            Ok(task_tools::publish_message(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_list_remote" => Ok(openclaw_tools::openclaw_list_remote(state).await),
        "vox_openclaw_search_remote" => {
            Ok(openclaw_tools::openclaw_search_remote(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_import_skill" => {
            Ok(openclaw_tools::openclaw_import_skill(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_discover" => Ok(openclaw_tools::openclaw_discover(state).await),
        "vox_openclaw_health" => Ok(openclaw_tools::openclaw_health(state).await),
        "vox_openclaw_gateway_call" => {
            Ok(openclaw_tools::openclaw_gateway_call(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_subscriptions" => Ok(openclaw_tools::openclaw_subscriptions(state).await),
        "vox_openclaw_subscribe" => {
            Ok(openclaw_tools::openclaw_subscribe(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_unsubscribe" => {
            Ok(openclaw_tools::openclaw_unsubscribe(state, serde_json::from_value(args)?).await)
        }
        "vox_openclaw_notify" => {
            Ok(openclaw_tools::openclaw_notify(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_list_remote" => Ok(agent_tools::agent_list_remote(state).await),
        "vox_agent_gateway_call" => {
            Ok(agent_tools::agent_gateway_call(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_subscriptions" => Ok(agent_tools::agent_subscriptions(state).await),
        "vox_agent_subscribe" => {
            Ok(agent_tools::agent_subscribe(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_unsubscribe" => {
            Ok(agent_tools::agent_unsubscribe(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_notify" => {
            Ok(agent_tools::agent_notify(state, serde_json::from_value(args)?).await)
        }

        "vox_git_log" => Ok(git_tools::git_log(
            state,
            args.get("max_commits")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize),
        )
        .await),
        "vox_git_diff" => {
            Ok(git_tools::git_diff(state, args.get("path").and_then(|v| v.as_str())).await)
        }
        "vox_git_status" => Ok(git_tools::git_status(state).await),
        "vox_git_blame" => Ok(git_tools::git_blame(
            state,
            args.get("path").and_then(|v| v.as_str()).unwrap_or("."),
        )
        .await),
        "vox_repo_index_status" => Ok(repo_index::repo_index_status(state).await),
        "vox_repo_index_refresh" => Ok(repo_index::repo_index_refresh(state).await),
        #[cfg(feature = "gui-visual-review")]
        "vox_visus_audit" => {
            Ok(visus_tools::vox_visus_audit(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "gui-visual-review")]
        "vox_visus_baseline" => {
            Ok(visus_tools::vox_visus_baseline(state, serde_json::from_value(args)?).await)
        }
        "vox_repo_status" => Ok(repo_catalog_tools::repo_status(state).await),
        "vox_gui_components" => {
            Ok(crate::gui_registry_tools::vox_gui_components(state, args).await)
        }
        "vox_gui_tokens" => Ok(crate::gui_registry_tools::vox_gui_tokens(state, args).await),
        "vox_validate_vuv" => Ok(crate::gui_registry_tools::vox_validate_vuv(state, args).await),
        "vox_gui_rules" => Ok(crate::gui_registry_tools::vox_gui_rules(state, args).await),
        "vox_agy_doctor" => Ok(crate::agy_tools::vox_agy_doctor(state, args).await),
        "vox_agy_delegate" => Ok(crate::agy_tools::vox_agy_delegate(state, args).await),
        "vox_agy_delegate_batch" => Ok(crate::agy_tools::vox_agy_delegate_batch(state, args).await),
        "vox_credentials_status" => Ok(crate::agy_tools::vox_credentials_status(state, args).await),
        "vox_agy_pipeline" => Ok(crate::agy_pipeline::vox_agy_pipeline(state, args).await),
        "vox_agy_review" => Ok(crate::agy_pipeline::vox_agy_review(state, args).await),
        "vox_agy_ledger_digest" => {
            Ok(crate::agy_pipeline::vox_agy_ledger_digest(state, args).await)
        }
        "vox_search_status" => {
            Ok(crate::graph_tools::graphify_status(state, serde_json::from_value(args)?).await)
        }
        "vox_search_structural" => {
            Ok(crate::graph_tools::graphify_search(state, serde_json::from_value(args)?).await)
        }
        "vox_search_neighbors" => {
            Ok(crate::graph_tools::graphify_query(state, serde_json::from_value(args)?).await)
        }
        "vox_search_path" => {
            Ok(crate::graph_tools::graphify_path(state, serde_json::from_value(args)?).await)
        }
        "vox_search_compare" => {
            Ok(crate::graph_tools::graphify_compare(state, serde_json::from_value(args)?).await)
        }
        "vox_search_rebuild" => {
            Ok(crate::graph_tools::graphify_rebuild(state, serde_json::from_value(args)?).await)
        }
        "vox_project_init" => Ok(project_init_tools::project_init(state, args).await),
        "vox_repo_catalog_list" => Ok(repo_catalog_tools::repo_catalog_list(state).await),
        "vox_repo_catalog_refresh" => Ok(repo_catalog_tools::repo_catalog_refresh(state).await),
        "vox_repo_query_text" => {
            Ok(repo_catalog_tools::repo_query_text(state, serde_json::from_value(args)?).await)
        }
        "vox_repo_query_file" => {
            Ok(repo_catalog_tools::repo_query_file(state, serde_json::from_value(args)?).await)
        }
        "vox_repo_query_history" => {
            Ok(repo_catalog_tools::repo_query_history(state, serde_json::from_value(args)?).await)
        }

        "vox_language_surface" => Ok(introspection_tools::language_surface().to_string()),
        "vox_capability_model_manifest" => {
            Ok(introspection_tools::capability_model_manifest(state)?.to_string())
        }
        "vox_compiler::ast_inspect" => Ok(introspection_tools::ast_inspect(
            state,
            args.get("path").and_then(|v| v.as_str()).unwrap_or("."),
        )
        .await?
        .to_string()),
        "vox_pipeline_status" => Ok(introspection_tools::pipeline_status().await.to_string()),
        "vox_decorator_registry" => Ok(introspection_tools::decorator_registry().to_string()),
        "vox_builtin_registry" => Ok(introspection_tools::builtin_registry().to_string()),
        "vox_workspace_modules" => Ok(introspection_tools::workspace_modules(state)
            .await?
            .to_string()),
        "vox_a2a_tasks" => Ok(introspection_tools::a2a_tasks(state).await?.to_string()),
        "vox_export_grammar_ebnf" => Ok(grammar_tools::export_grammar_ebnf(state).await),

        "vox_snapshot_list" => Ok(vcs_tools::snapshot_list(state, args).await),
        "vox_snapshot_diff" => Ok(vcs_tools::snapshot_diff(state, args).await),
        "vox_snapshot_restore" => Ok(vcs_tools::snapshot_restore(state, args).await),
        "vox_oplog" => Ok(vcs_tools::oplog_list(state, args).await),
        "vox_undo" => Ok(vcs_tools::oplog_undo(state, args).await),
        "vox_redo" => Ok(vcs_tools::oplog_redo(state, args).await),
        "vox_conflicts" => Ok(vcs_tools::conflicts_list(state).await),
        "vox_resolve_conflict" => Ok(vcs_tools::resolve_conflict(state, args).await),
        "vox_conflict_diff" => Ok(vcs_tools::conflict_diff(state, args).await),
        "vox_workspace_create" => Ok(vcs_tools::workspace_create(state, args).await),
        "vox_workspace_merge" => Ok(vcs_tools::workspace_merge(state, args).await),
        "vox_workspace_status" => Ok(vcs_tools::workspace_status(state, args).await),
        "vox_change_create" => Ok(vcs_tools::change_create(state, args).await),
        "vox_change_log" => Ok(vcs_tools::change_log(state, args).await),
        "vox_vcs_status" => Ok(crate::dei_tools::vcs_status(state).await),

        "vox_db_schema" => Ok(db_tools::vox_db_schema(args)),
        "vox_db_relationships" => Ok(db_tools::vox_db_relationships(args)),
        "vox_db_data_flow" => Ok(db_tools::vox_db_data_flow(args)),
        "vox_db_sample_data" => Ok(db_tools::vox_db_sample_data(state, args).await),
        "vox_journey_canonical_steps" => {
            Ok(db_tools::vox_journey_canonical_steps(state, args).await)
        }
        "vox_db_explain_query" => Ok(db_tools::vox_db_explain_query(state, args).await),
        "vox_db_suggest_query" => Ok(db_tools::vox_db_suggest_query(state, args).await),
        "vox_secrets_doctor" => Ok(secrets_tools::secrets_doctor(state, args).await),

        "vox_db_research_session_upsert" => {
            Ok(codex_tools::codex_research_session_upsert(state, args).await)
        }
        "vox_db_conversation_version_append" => {
            Ok(codex_tools::codex_conversation_version_append(state, args).await)
        }
        "vox_db_conversation_edge_insert" => {
            Ok(codex_tools::codex_conversation_edge_insert(state, args).await)
        }
        "vox_db_topic_evolution_append" => {
            Ok(codex_tools::codex_topic_evolution_append(state, args).await)
        }
        "vox_db_research_metric_linked" => {
            Ok(codex_tools::codex_research_metric_linked(state, args).await)
        }
        "vox_db_trust_rollups" => Ok(trust_tools::trust_rollups_list(state, args).await),
        "vox_db_trust_summary" => Ok(trust_tools::trust_rollups_summary(state, args).await),
        "vox_db_trust_drift" => Ok(trust_tools::trust_observation_drift(state, args).await),
        "vox_db_trust_propagate" => Ok(trust_tools::trust_propagate(state, args).await),

        "vox_generate_code" => Ok(compiler_tools::generate_vox_code(state, args).await),
        #[cfg(feature = "oratio-rerank")]
        "vox_speech_to_code" => Ok(speech_pipeline_tools::speech_to_code(state, args).await?),
        "vox_list_models" => {
            Ok(crate::models::list_models(state, serde_json::from_value(args)?).await)
        }
        "vox_suggest_model" => {
            Ok(crate::models::suggest_model(state, serde_json::from_value(args)?).await)
        }
        "vox_set_model" => Ok(crate::models::set_model(state, serde_json::from_value(args)?).await),
        "vox_set_active_model" => Ok(crate::models::set_active_mcp_chat_model(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        "vox_get_active_model" => Ok(crate::models::get_active_mcp_chat_model(state).await),
        "vox_build_crate" => Ok(compiler_tools::build_crate(
            state,
            args.get("crate_name").and_then(|v| v.as_str()),
        )
        .await),
        "vox_lint_crate" => Ok(compiler_tools::lint_crate(
            state,
            args.get("crate_name").and_then(|v| v.as_str()),
        )
        .await),
        "vox_coverage_report" => Ok(compiler_tools::coverage_report(
            state,
            args.get("crate_name").and_then(|v| v.as_str()),
        )
        .await),

        // Execution Budget
        "vox_exec_time_query" => Ok(exec_time_tools::exec_time_query(state, args).await),
        "vox_exec_time_record" => Ok(exec_time_tools::exec_time_record(state, args).await),

        // ── Chat & Inline AI ──────────────────────────────────────────────
        "vox_chat_message" => {
            Ok(chat_tools::chat_message(state, serde_json::from_value(args)?).await)
        }
        "vox_chat_history" => {
            Ok(chat_tools::chat_history(state, serde_json::from_value(args)?).await)
        }
        "vox_questioning_submit_answer" => Ok(questioning_tools::questioning_submit_answer(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        "vox_questioning_pending" => {
            Ok(questioning_tools::questioning_pending(state, serde_json::from_value(args)?).await)
        }
        "vox_questioning_sync_ssot" => Ok(questioning_tools::questioning_sync_ssot(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        "vox_inline_edit" => {
            Ok(chat_tools::inline_edit(state, serde_json::from_value(args)?).await)
        }
        "vox_apply_structured_edit" => Ok(compiler_tools::apply_structured_edit(state, args).await),
        "vox_plan" => Ok(chat_tools::plan_goal(state, serde_json::from_value(args)?).await),
        "vox_replan" => Ok(chat_tools::plan_replan(state, serde_json::from_value(args)?).await),
        "vox_plan_status" => {
            Ok(chat_tools::plan_status(state, serde_json::from_value(args)?).await)
        }
        "vox_plan_list_sessions" => {
            Ok(chat_tools::plan_list_sessions(state, serde_json::from_value(args)?).await)
        }
        "vox_plan_resume" => {
            Ok(chat_tools::plan_resume(state, serde_json::from_value(args)?).await)
        }
        "vox_ghost_text" => Ok(chat_tools::ghost_text(state, serde_json::from_value(args)?).await),

        "vox_schola_submit" => {
            Ok(training_tools::train_submit(state, serde_json::from_value(args)?).await)
        }

        #[cfg(feature = "news-publish")]
        "vox_news_test_syndicate" => {
            Ok(news_tools::vox_news_test_syndicate(state, serde_json::from_value(args)?).await)
        }

        #[cfg(feature = "news-publish")]
        "vox_news_draft_research" => {
            Ok(news_tools::vox_news_draft_research(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "news-publish")]
        "vox_news_approve" => {
            Ok(news_tools::vox_news_approve(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "news-publish")]
        "vox_news_approval_status" => {
            Ok(news_tools::vox_news_approval_status(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "news-publish")]
        "vox_news_simulate_publish_gate" => Ok(news_tools::vox_news_simulate_publish_gate(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_prepare" => Ok(scientia_tools::vox_scientia_publication_prepare(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_approve" => Ok(scientia_tools::vox_scientia_publication_approve(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_submit_local" => {
            Ok(scientia_tools::vox_scientia_publication_submit_local(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_status" => Ok(scientia_tools::vox_scientia_publication_status(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_scholarly_remote_status" => Ok(
            scientia_tools::vox_scientia_publication_scholarly_remote_status(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_scholarly_remote_status_sync_all" => Ok(
            scientia_tools::vox_scientia_publication_scholarly_remote_status_sync_all(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_scholarly_remote_status_sync_batch" => Ok(
            scientia_tools::vox_scientia_publication_scholarly_remote_status_sync_batch(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_arxiv_handoff_record" => Ok(
            scientia_tools::vox_scientia_publication_arxiv_handoff_record(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_scholarly_staging_export" => Ok(
            scientia_tools::vox_scientia_publication_scholarly_staging_export(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_external_jobs_due" => {
            Ok(scientia_tools::vox_scientia_publication_external_jobs_due(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_external_jobs_dead_letter" => Ok(
            scientia_tools::vox_scientia_publication_external_jobs_dead_letter(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_external_jobs_replay" => Ok(
            scientia_tools::vox_scientia_publication_external_jobs_replay(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_external_jobs_tick" => {
            Ok(scientia_tools::vox_scientia_publication_external_jobs_tick(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_external_pipeline_metrics" => Ok(
            scientia_tools::vox_scientia_publication_external_pipeline_metrics(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_scholarly_pipeline_run" => Ok(
            scientia_tools::vox_scientia_publication_scholarly_pipeline_run(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_media_upsert" => {
            Ok(scientia_tools::vox_scientia_publication_media_upsert(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_media_list" => {
            Ok(scientia_tools::vox_scientia_publication_media_list(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_media_delete" => {
            Ok(scientia_tools::vox_scientia_publication_media_delete(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_route_simulate" => {
            Ok(scientia_tools::vox_scientia_publication_route_simulate(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_publish" => Ok(scientia_tools::vox_scientia_publication_publish(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_retry_failed" => {
            Ok(scientia_tools::vox_scientia_publication_retry_failed(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_preflight" => {
            Ok(scientia_tools::vox_scientia_publication_preflight(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_worthiness_evaluate" => Ok(scientia_tools::vox_scientia_worthiness_evaluate(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_discovery_scan" => {
            Ok(scientia_tools::vox_scientia_publication_discovery_scan(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_discovery_explain" => {
            Ok(scientia_tools::vox_scientia_publication_discovery_explain(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_discovery_refresh_evidence" => Ok(
            scientia_tools::vox_scientia_publication_discovery_refresh_evidence(
                state,
                serde_json::from_value(args)?,
            )
            .await,
        ),
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_novelty_fetch" => {
            Ok(scientia_tools::vox_scientia_publication_novelty_fetch(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_decision_explain" => {
            Ok(scientia_tools::vox_scientia_publication_decision_explain(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_publication_novelty_happy_path" => {
            Ok(scientia_tools::vox_scientia_publication_novelty_happy_path(
                state,
                serde_json::from_value(args)?,
            )
            .await)
        }
        #[cfg(feature = "news-publish")]
        "vox_scientia_assist_suggestions" => Ok(scientia_tools::vox_scientia_assist_suggestions(
            state,
            serde_json::from_value(args)?,
        )
        .await),

        // Delegate others to existing modules
        "vox_my_files" => Ok(crate::affinity::my_files(state, serde_json::from_value(args)?).await),
        "vox_claim_file" => {
            Ok(crate::affinity::claim_file(state, serde_json::from_value(args)?).await)
        }
        "vox_transfer_file" => {
            Ok(crate::affinity::transfer_file(state, serde_json::from_value(args)?).await)
        }

        "vox_ask_agent" => Ok(crate::qa::ask_agent(state, serde_json::from_value(args)?).await),
        "vox_answer_question" => {
            Ok(crate::qa::answer_question(state, serde_json::from_value(args)?).await)
        }
        "vox_pending_questions" => {
            Ok(crate::qa::pending_questions(state, serde_json::from_value(args)?).await)
        }
        "vox_broadcast" => Ok(crate::qa::broadcast(state, serde_json::from_value(args)?).await),

        "vox_memory_store" => {
            Ok(crate::memory::memory_store(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_recall" => {
            Ok(crate::memory::memory_recall(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_search" => {
            Ok(crate::memory::memory_search(state, serde_json::from_value(args)?).await)
        }
        "vox_semantic_fs_discover" => {
            Ok(crate::memory::semantic_fs_discover_mcp(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_log" => {
            Ok(crate::memory::memory_daily_log(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_list_keys" => Ok(crate::memory::memory_list_keys(state).await),
        "vox_knowledge_query" => {
            Ok(crate::memory::knowledge_query(state, serde_json::from_value(args)?).await)
        }
        "vox_research_run" => {
            Ok(crate::memory::research_run(state, serde_json::from_value(args)?).await)
        }
        "vox_research_start" => {
            Ok(crate::memory::research_start(state, serde_json::from_value(args)?).await)
        }
        "vox_research_status" => {
            Ok(crate::memory::research_status(state, serde_json::from_value(args)?).await)
        }
        "vox_research_get" => {
            Ok(crate::memory::research_get(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_save_db" => {
            Ok(crate::memory::memory_save_db(state, serde_json::from_value(args)?).await)
        }
        "vox_memory_recall_db" => {
            Ok(crate::memory::memory_recall_db(state, serde_json::from_value(args)?).await)
        }

        // ── Knowledge Bases (VoxKB) ──────────────────────────────────────────
        "vox_kb_create" => Ok(crate::kb::kb_create(state, serde_json::from_value(args)?).await),
        "vox_kb_list" => Ok(crate::kb::kb_list(state).await),
        "vox_kb_delete" => Ok(crate::kb::kb_delete(state, serde_json::from_value(args)?).await),
        "vox_kb_add_entry" => {
            Ok(crate::kb::kb_add_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_delete_entry" => {
            Ok(crate::kb::kb_delete_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_list_entries" => {
            Ok(crate::kb::kb_list_entries(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_review_entry" => {
            Ok(crate::kb::kb_review_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_get_feed" => Ok(crate::kb::kb_get_feed(state, serde_json::from_value(args)?).await),
        "vox_kb_add_rule" => Ok(crate::kb::kb_add_rule(state, serde_json::from_value(args)?).await),
        "vox_kb_list_rules" => {
            Ok(crate::kb::kb_list_rules(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_query" => Ok(crate::kb::kb_query(state, serde_json::from_value(args)?).await),
        "vox_kb_clip" => Ok(crate::kb::kb_clip(state, serde_json::from_value(args)?).await),

        "vox_compaction_status" => {
            Ok(crate::memory::compaction_status(state, serde_json::from_value(args)?).await)
        }
        "vox_session_create" => {
            Ok(crate::memory::session_create(state, serde_json::from_value(args)?).await)
        }
        "vox_session_list" => Ok(crate::memory::session_list(state).await),
        "vox_session_reset" => {
            Ok(crate::memory::session_reset(state, serde_json::from_value(args)?).await)
        }
        "vox_session_compact" => {
            Ok(crate::memory::session_compact(state, serde_json::from_value(args)?).await)
        }
        "vox_session_info" => {
            Ok(crate::memory::session_info(state, serde_json::from_value(args)?).await)
        }
        "vox_session_cleanup" => Ok(crate::memory::session_cleanup(state).await),

        "vox_preference_get" => {
            Ok(crate::memory::preference_get(state, serde_json::from_value(args)?).await)
        }
        "vox_preference_set" => {
            Ok(crate::memory::preference_set(state, serde_json::from_value(args)?).await)
        }
        "vox_preference_list" => {
            Ok(crate::memory::preference_list(state, serde_json::from_value(args)?).await)
        }
        "vox_learn_pattern" => {
            Ok(crate::memory::learn_pattern(state, serde_json::from_value(args)?).await)
        }
        "vox_behavior_record" => {
            Ok(crate::memory::behavior_record(state, serde_json::from_value(args)?).await)
        }
        "vox_behavior_summary" => {
            Ok(crate::memory::behavior_summary(state, serde_json::from_value(args)?).await)
        }

        "vox_check_mood" => {
            Ok(crate::gamify::check_mood(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_notifications_list" => {
            Ok(crate::gamify::ludus_notifications_list(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_progress_snapshot" => {
            Ok(crate::gamify::ludus_progress_snapshot(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_notification_ack" => {
            Ok(crate::gamify::ludus_notification_ack(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_notifications_ack_all" => {
            Ok(crate::gamify::ludus_notifications_ack_all(state).await)
        }
        "vox_gamify_quest_list" => {
            Ok(crate::gamify::ludus_quest_list(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_shop_catalog" => {
            Ok(crate::gamify::ludus_shop_catalog(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_shop_buy" => {
            Ok(crate::gamify::ludus_shop_buy(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_collegium_join" => {
            Ok(crate::gamify::ludus_collegium_join(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_battle_start" => {
            Ok(crate::gamify::ludus_battle_start(state, serde_json::from_value(args)?).await)
        }
        "vox_gamify_battle_submit" => {
            Ok(crate::gamify::ludus_battle_submit(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_status" => {
            Ok(crate::gamify::agent_status(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_continue" => {
            Ok(crate::gamify::agent_continue(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_assess" => {
            Ok(crate::gamify::agent_assess(state, serde_json::from_value(args)?).await)
        }
        "vox_agent_handoff" => {
            Ok(crate::gamify::agent_handoff(state, serde_json::from_value(args)?).await)
        }

        "vox_queue_status" => {
            Ok(crate::dei_tools::queue_status(state, serde_json::from_value(args)?).await)
        }
        "vox_lock_status" => Ok(crate::dei_tools::lock_status(state).await),
        "vox_budget_status" => Ok(crate::dei_tools::budget_status(state).await),
        "vox_attention_summary" => {
            Ok(crate::dei_tools::attention_summary(state, serde_json::from_value(args)?).await)
        }
        "vox_attention_history" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let bm = state.orchestrator.budget_manager_handle();
            let events =
                vox_orchestrator::sync_lock::rw_read(&*bm).attention_events_snapshot(limit);
            Ok(crate::params::ToolResult::ok(serde_json::to_value(&events)?).to_json())
        }
        "vox_attention_reset" => {
            let bm = state.orchestrator.budget_manager_handle();
            vox_orchestrator::sync_lock::rw_read(&*bm).reset_attention();
            // T-001: Also reset MCP-level Socrates attention tracking
            state.reset_all_questioning_attention();
            Ok(crate::params::ToolResult::ok(serde_json::json!({
                "reset": true,
                "message": "Attention budget spend and Socrates focus zeroed process-wide."
            }))
            .to_json())
        }
        "vox_trust_override" => {
            let agent_id = args
                .get("agent_id")
                .and_then(|v| v.as_u64())
                .map(|id| vox_orchestrator::types::AgentId(id as _))
                .unwrap_or(vox_orchestrator::types::AgentId(0));
            let trust_score = args
                .get("trust_score")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let bm = state.orchestrator.budget_manager_handle();
            vox_orchestrator::sync_lock::rw_read(&*bm).force_trust_score(agent_id, trust_score);
            Ok(crate::params::ToolResult::ok(serde_json::json!({
                "agent_id": agent_id.0,
                "trust_score": trust_score,
                "message": "Trust score overridden."
            }))
            .to_json())
        }
        "vox_handoff_lineage" => {
            Ok(crate::dei_tools::handoff_lineage(state, serde_json::from_value(args)?).await)
        }
        "vox_cancel_task" => {
            Ok(crate::dei_tools::cancel_task(state, serde_json::from_value(args)?).await)
        }
        "vox_reorder_task" => {
            Ok(crate::dei_tools::reorder_task(state, serde_json::from_value(args)?).await)
        }
        "vox_drain_agent" => {
            Ok(crate::dei_tools::drain_agent(state, serde_json::from_value(args)?).await)
        }
        "vox_cost_history" => {
            Ok(crate::dei_tools::cost_history(state, serde_json::from_value(args)?).await)
        }
        "vox_file_graph" => Ok(crate::dei_tools::file_graph(state).await),
        "vox_config_get" => Ok(crate::dei_tools::config_get(state).await),
        "vox_config_set" => Ok(crate::dei_tools::config_set(state, args).await),
        "vox_map_agent_session" => {
            Ok(crate::dei_tools::map_agent_session(state, serde_json::from_value(args)?).await)
        }
        "vox_poll_events" => {
            Ok(crate::dei_tools::poll_events(state, serde_json::from_value(args)?).await)
        }
        "vox_heartbeat" => {
            Ok(crate::dei_tools::heartbeat(state, serde_json::from_value(args)?).await)
        }
        "vox_record_cost" => {
            Ok(crate::dei_tools::record_cost(state, serde_json::from_value(args)?).await)
        }
        "vox_rebalance" => Ok(crate::dei_tools::rebalance(state).await),
        "vox_agent_events" => {
            Ok(crate::dei_tools::agent_events(state, serde_json::from_value(args)?).await)
        }

        "vox_a2a_send" => {
            Ok(crate::a2a_tools::a2a_send(state, serde_json::from_value(args)?).await)
        }
        "vox_a2a_inbox" => {
            Ok(crate::a2a_tools::a2a_inbox(state, serde_json::from_value(args)?).await)
        }
        "vox_a2a_ack" => Ok(crate::a2a_tools::a2a_ack(state, serde_json::from_value(args)?).await),
        "vox_a2a_broadcast" => {
            Ok(crate::a2a_tools::a2a_broadcast(state, serde_json::from_value(args)?).await)
        }
        "vox_a2a_history" => {
            Ok(crate::a2a_tools::a2a_history(state, serde_json::from_value(args)?).await)
        }

        "vox_skill_install" => {
            Ok(crate::skills::skill_install(state, serde_json::from_value(args)?).await)
        }
        "vox_skill_uninstall" => {
            Ok(crate::skills::skill_uninstall(state, serde_json::from_value(args)?).await)
        }
        "vox_skill_list" => Ok(crate::skills::skill_list(state)),
        "vox_skill_search" => Ok(crate::skills::skill_search(
            state,
            serde_json::from_value(args)?,
        )),
        "vox_skill_info" => Ok(crate::skills::skill_info(
            state,
            serde_json::from_value(args)?,
        )),
        "vox_skill_parse" => Ok(crate::skills::skill_parse(serde_json::from_value(args)?)),
        "vox_skill_use" => Ok(crate::skills::skill_use(
            state,
            serde_json::from_value(args)?,
        )),
        "vox_skill_run" => Ok(crate::skills::skill_run(state, serde_json::from_value(args)?).await),
        "vox_skill_discover" => Ok(crate::skills::skill_discover(state)),
        "vox_skill_add" => Ok(crate::skills::skill_add(state, serde_json::from_value(args)?).await),
        "vox_skill_remove" => {
            Ok(crate::skills::skill_remove(state, serde_json::from_value(args)?).await)
        }

        "vox_workspace_mcp_refresh" => {
            let root = state
                .workspace_root
                .clone()
                .unwrap_or_else(|| state.repository.root.clone());
            let config = crate::workspace_mcp::load_scan_config(&root);
            let load = crate::workspace_mcp::WorkspaceMcpLoader::load_repo(&root, &config)
                .map_err(|e| anyhow::anyhow!(e))?;
            for err in &load.errors {
                tracing::warn!(
                    path = %err.path.display(),
                    error = %err.message,
                    "workspace MCP scan skipped file"
                );
            }
            if !load.surface.shadowed.is_empty() {
                tracing::warn!(
                    shadowed = ?load.surface.shadowed,
                    "workspace MCP tools shadowed by static catalog"
                );
            }
            *state.workspace_mcp.write() = load.surface.clone();
            let errors: Vec<serde_json::Value> = load
                .errors
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "path": e.path.display().to_string(),
                        "message": e.message,
                    })
                })
                .collect();
            Ok(ToolResult::ok(serde_json::json!({
                "tool_count": load.surface.tool_count(),
                "resource_count": load.surface.resource_count(),
                "shadowed": load.surface.shadowed,
                "duplicate_tools": load.surface.duplicate_tools,
                "duplicate_resources": load.surface.duplicate_resources,
                "errors": errors,
            }))
            .to_json())
        }

        "vox_plugin_list" => Ok(crate::plugins::plugin_list(state).await),
        "vox_plugin_catalog" => Ok(crate::plugins::plugin_catalog()),
        "vox_plugin_info" => {
            Ok(crate::plugins::plugin_info(state, serde_json::from_value(args)?).await)
        }
        "vox_plugin_install" => {
            Ok(crate::plugins::plugin_install(state, serde_json::from_value(args)?).await)
        }
        "vox_plugin_remove" => {
            Ok(crate::plugins::plugin_remove(state, serde_json::from_value(args)?).await)
        }

        "vox_set_context" => {
            Ok(crate::mcp_context::set_context(state, serde_json::from_value(args)?).await)
        }
        "vox_get_context" => {
            Ok(crate::mcp_context::get_context(state, serde_json::from_value(args)?).await)
        }
        "vox_list_context" => {
            Ok(crate::mcp_context::list_context(state, serde_json::from_value(args)?).await)
        }
        "vox_context_budget" => {
            Ok(crate::mcp_context::context_budget(state, serde_json::from_value(args)?).await)
        }
        "vox_set_agent_budget" => {
            Ok(crate::mcp_context::set_agent_budget(state, serde_json::from_value(args)?).await)
        }
        "vox_emergency_stop" => {
            Ok(crate::mcp_context::emergency_stop(state, serde_json::from_value(args)?).await)
        }
        "vox_handoff_context" => {
            Ok(crate::mcp_context::handoff_context(state, serde_json::from_value(args)?).await)
        }

        #[cfg(feature = "oratio-rerank")]
        "vox_oratio_transcribe" => Ok(oratio_tools::transcribe(state, args)?),
        #[cfg(feature = "oratio-rerank")]
        "vox_oratio_listen" => Ok(oratio_tools::listen(state, args).await?),
        #[cfg(feature = "oratio-rerank")]
        "vox_oratio_status" => Ok(oratio_tools::status()),

        "vox_populi_local_status" => Ok(populi_tools::mesh_local_status(args)?),
        "vox_mesh_nodes" => Ok(populi_tools::mesh_nodes(state, args).await?),
        "vox_mesh_queue_stats" => Ok(populi_tools::mesh_queue_stats(state, args).await?),
        "vox_mesh_dispatch" => Ok(populi_tools::mesh_dispatch(state, args).await?),

        #[cfg(feature = "heavy-browser")]
        "vox_browser_open" => {
            Ok(browser_tools::browser_open(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_list_pages" => Ok(browser_tools::browser_list_pages(state, args).await),
        #[cfg(feature = "heavy-browser")]
        "vox_browser_page_info" => {
            Ok(browser_tools::browser_page_info(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_close" => {
            Ok(browser_tools::browser_close(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_back" => {
            Ok(browser_tools::browser_back(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_forward" => {
            Ok(browser_tools::browser_forward(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_reload" => {
            Ok(browser_tools::browser_reload(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_stop" => {
            Ok(browser_tools::browser_stop(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_goto" => {
            Ok(browser_tools::browser_goto(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_click" => {
            Ok(browser_tools::browser_click(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_click_xy" => {
            Ok(browser_tools::browser_click_xy(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_fill" => {
            Ok(browser_tools::browser_fill(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_scroll" => {
            Ok(browser_tools::browser_scroll(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_press" => {
            Ok(browser_tools::browser_press(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_type" => {
            Ok(browser_tools::browser_type(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_set_viewport" => {
            Ok(browser_tools::browser_set_viewport(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_set_control_lock" => {
            Ok(browser_tools::browser_set_control_lock(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_wait_for" => {
            Ok(browser_tools::browser_wait_for(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_text" => {
            Ok(browser_tools::browser_text(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_html" => {
            Ok(browser_tools::browser_html(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_screenshot" => {
            Ok(browser_tools::browser_screenshot(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_screenshot_viewport" => Ok(browser_tools::browser_screenshot_viewport(
            state,
            serde_json::from_value(args)?,
        )
        .await),
        #[cfg(feature = "heavy-browser")]
        "vox_browser_screencast_frame" => {
            Ok(browser_tools::browser_screencast_frame(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_extract" => {
            Ok(browser_tools::browser_extract(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_extract_json" => {
            Ok(browser_tools::browser_extract_json(state, serde_json::from_value(args)?).await)
        }
        #[cfg(feature = "heavy-browser")]
        "vox_browser_act" => {
            Ok(browser_tools::browser_act(state, serde_json::from_value(args)?).await)
        }

        "vox_benchmark_list" => {
            Ok(benchmark_tools::benchmark_list(state, serde_json::from_value(args)?).await)
        }
        "vox_benchmark_record" => {
            Ok(benchmark_tools::benchmark_record(state, serde_json::from_value(args)?).await)
        }
        "vox_code_audit_findings_upsert" => {
            Ok(toestub_tools::toestub_findings_upsert(state, serde_json::from_value(args)?).await)
        }
        _ => {
            // Check skill macro tools
            let skills = state.skill_registry.list(None);
            if let Some(skill) = skills.iter().find(|s| s.tools.contains(&name.to_string())) {
                if let Some(db) = &state.db {
                    if let Ok(Some(entry)) = db.get_skill_manifest(&skill.id).await {
                        let msg = format!(
                            "This tool is an instructional macro from skill '{}'.\n\nPlease read these instructions and perform the requested actions yourself:\n\n{}",
                            skill.name, entry.skill_md
                        );
                        return Ok(ToolResult::ok(msg).to_json());
                    }
                }
            }
            Err(anyhow::anyhow!("Unknown tool: {}", name))
        }
    }
}

// ── Track E telemetry helpers ──────────────────────────────────────────────

fn tool_call_kind_for(name: &str) -> &'static str {
    if name.starts_with("vox_run") || name.starts_with("vox_exec") || name.starts_with("vox_shell")
    {
        "exec"
    } else if name.starts_with("vox_write")
        || name.starts_with("vox_patch")
        || name.starts_with("vox_inline_edit")
        || name.starts_with("vox_multi_replace")
        || name.starts_with("vox_delete")
    {
        "edit"
    } else if name.starts_with("vox_read")
        || name.starts_with("vox_list")
        || name.starts_with("vox_search")
    {
        "read"
    } else if name.starts_with("vox_agent")
        || name.starts_with("vox_submit_task")
        || name.starts_with("vox_task")
    {
        "orchestration"
    } else {
        "other"
    }
}

fn is_file_mutation(name: &str) -> bool {
    matches!(
        name,
        "vox_write_file"
            | "vox_patch_file"
            | "vox_inline_edit_file"
            | "vox_multi_replace"
            | "vox_multi_replace_file"
    )
}

fn file_op_type(name: &str) -> &'static str {
    match name {
        "vox_write_file" => "write",
        "vox_patch_file" => "patch",
        "vox_inline_edit_file" => "inline_edit",
        "vox_multi_replace" | "vox_multi_replace_file" => "multi_replace",
        _ => "other",
    }
}

fn file_kind_from_path(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "vox" => "vox",
        "html" | "htm" => "html",
        "css" | "scss" => "css",
        "" => "no_ext",
        _ => "other",
    }
}

fn content_size_bucket(len: usize) -> &'static str {
    match len {
        0..=511 => "lt512b",
        512..=4095 => "512b_to_4k",
        4096..=32767 => "4k_to_32k",
        32768..=262143 => "32k_to_256k",
        _ => "gt256k",
    }
}

fn error_class_from_err(e: &anyhow::Error) -> &'static str {
    let msg = format!("{e:?}");
    if msg.contains("permission") || msg.contains("unauthorized") || msg.contains("denied") {
        "permission"
    } else if msg.contains("not found") || msg.contains("No such file") {
        "not_found"
    } else if msg.contains("timeout") || msg.contains("timed out") {
        "timeout"
    } else if msg.contains("parse") || msg.contains("invalid") || msg.contains("deserialize") {
        "invalid_input"
    } else {
        "internal"
    }
}

fn subsystem_from_tool(name: &str) -> &'static str {
    if name.starts_with("vox_run") || name.starts_with("vox_exec") || name.starts_with("vox_shell")
    {
        "exec"
    } else if name.starts_with("vox_write")
        || name.starts_with("vox_patch")
        || name.starts_with("vox_inline_edit")
        || name.starts_with("vox_multi_replace")
    {
        "file_ops"
    } else if name.starts_with("vox_git") || name.starts_with("vox_vcs") {
        "vcs"
    } else if name.starts_with("vox_agent") || name.starts_with("vox_task") {
        "orchestration"
    } else if name.starts_with("vox_chat") || name.starts_with("vox_plan") {
        "chat"
    } else {
        "mcp"
    }
}

#[cfg(test)]
mod registry_dispatch_tests {
    use super::super::{TOOL_REGISTRY, handle_tool_call};
    use crate::server_state::ServerState;
    use serde_json::json;
    use std::collections::HashSet;

    /// Subprocess / full-workspace tools — do not invoke from this guard (CI time + host deps).
    const SKIP_DISPATCH_PROBE: &[&str] = &[
        "vox_check_workspace",
        "vox_test_all",
        "vox_run_tests",
        "vox_build_crate",
        "vox_lint_crate",
        "vox_coverage_report",
        "vox_validate_file",
        "vox_validate_source",
        "vox_generate_code",
        "vox_project_init",
        "vox_oratio_transcribe",
        "vox_oratio_listen",
        "vox_oratio_status",
        "vox_speech_to_code",
        "vox_openclaw_list_remote",
        "vox_openclaw_search_remote",
        "vox_openclaw_import_skill",
        "vox_openclaw_discover",
        "vox_openclaw_health",
        "vox_openclaw_gateway_call",
        "vox_openclaw_subscriptions",
        "vox_openclaw_subscribe",
        "vox_openclaw_unsubscribe",
        "vox_openclaw_notify",
        "vox_agent_list_remote",
        "vox_agent_gateway_call",
        "vox_agent_subscriptions",
        "vox_agent_subscribe",
        "vox_agent_unsubscribe",
        "vox_agent_notify",
        "vox_browser_open",
        "vox_browser_list_pages",
        "vox_browser_page_info",
        "vox_browser_close",
        "vox_browser_back",
        "vox_browser_forward",
        "vox_browser_reload",
        "vox_browser_stop",
        "vox_browser_goto",
        "vox_browser_click",
        "vox_browser_click_xy",
        "vox_browser_fill",
        "vox_browser_scroll",
        "vox_browser_press",
        "vox_browser_type",
        "vox_browser_set_viewport",
        "vox_browser_set_control_lock",
        "vox_browser_wait_for",
        "vox_browser_text",
        "vox_browser_html",
        "vox_browser_screenshot",
        "vox_browser_screenshot_viewport",
        "vox_browser_screencast_frame",
        "vox_browser_extract",
        "vox_browser_extract_json",
        "vox_browser_act",
        // T0.3: always_requires_approval — parks unconditionally under every
        // PermissionMode (including accept_all) and is never satisfied by
        // the persisted allowlist (see permission_modes::RISK_CLASSES /
        // dispatch.rs's dangerous-tool gate). Probing it with `handle_tool_call`
        // (mode = None -> Ask) here would register a pending approval that
        // nothing in this test ever resolves, burning the full 300s
        // APPROVAL_TIMEOUT before falling through as TimedOut — under
        // nextest's slow-timeout profile this test gets killed and retried
        // long before that, wasting several minutes of CI time per run.
        "vox_add_approval_allowlist_entry",
    ];

    #[tokio::test]
    async fn tool_registry_names_are_unique() {
        let mut seen = HashSet::new();
        for e in TOOL_REGISTRY {
            let name = e.name;
            assert!(seen.insert(name), "duplicate TOOL_REGISTRY name: {name}");
        }
    }

    #[test]
    fn yaml_registry_tools_have_dispatch_match_arms() {
        let src = include_str!("dispatch.rs");
        for e in TOOL_REGISTRY {
            let needle = format!("\"{}\" =>", e.name);
            assert!(
                src.contains(&needle),
                "TOOL_REGISTRY entry `{}` must have a `match` arm in dispatch.rs (SSOT: contracts/mcp/tool-registry.canonical.yaml)",
                e.name
            );
        }
    }

    #[tokio::test]
    async fn every_registry_tool_has_static_dispatch() {
        let state = ServerState::new_test().await;
        for e in TOOL_REGISTRY {
            let name = e.name;
            if SKIP_DISPATCH_PROBE.contains(&name) {
                continue;
            }
            // Tools whose dispatch arm is feature-gated out under the current build are
            // not dispatchable and are also filtered from the advertised registry; skip
            // them here so the probe matches the compiled dispatch surface.
            if !crate::registry::dispatchable_under_features(name) {
                continue;
            }
            let res = handle_tool_call(&state, name, json!({})).await;
            if let Err(e) = res {
                assert!(
                    !e.to_string().contains("Unknown tool"),
                    "missing dispatch for {name}: {e}"
                );
            }
        }
    }
}

// ── T4.3: per-tool-call dispatch timeout tests ──────────────────────────────
#[cfg(test)]
mod dispatch_timeout_tests {
    use super::handle_tool_call;
    use crate::server_state::ServerState;
    use serde_json::json;

    /// RED test 1: a deliberately-hanging tool returns a timeout error
    /// envelope after its configured (short, test-only) duration, and the
    /// handler remains responsive for a subsequent normal call — proving the
    /// outer `tokio::time::timeout` in `handle_tool_call_with_mode` actually
    /// aborts execution rather than leaving the connection hanging, and that
    /// it doesn't leave any shared state (locks, `ServerState`) poisoned.
    #[tokio::test]
    async fn hanging_tool_times_out_and_handler_stays_responsive() {
        let state = ServerState::new_test().await;

        let started = std::time::Instant::now();
        let res = handle_tool_call(&state, "vox_test_hang_forever", json!({}))
            .await
            .expect("dispatch itself must not error; timeout is surfaced as a ToolResult envelope");
        let elapsed = started.elapsed();

        assert!(
            crate::server_state::tool_json_envelope_is_error(&res),
            "expected a `success: false` timeout envelope, got: {res}"
        );
        assert!(
            res.contains("exceeded its execution timeout"),
            "expected the timeout error message, got: {res}"
        );
        // The configured test timeout is 200ms; give generous CI slack but
        // assert it did NOT wait anywhere near the (effectively infinite)
        // sleep duration the test tool would otherwise run for.
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "timeout took far too long to fire: {elapsed:?}"
        );

        // Handler must still be responsive afterward — prove no poisoned
        // lock / broken state by making an ordinary, fast call right after.
        let follow_up = handle_tool_call(&state, "vox_skill_list", json!({}))
            .await
            .expect("handler must remain responsive after a prior call timed out");
        assert!(
            !crate::server_state::tool_json_envelope_is_error(&follow_up),
            "follow-up call unexpectedly failed: {follow_up}"
        );
    }

    /// RED test 2: an agy-delegation test-double that sleeps LONGER than the
    /// global default timeout but SHORTER than its own configured long
    /// timeout completes successfully — proving the agy exception is a real,
    /// effective per-tool override and not just documented.
    #[tokio::test]
    async fn agy_like_tool_survives_past_the_global_default_timeout() {
        let state = ServerState::new_test().await;

        // Global default (dispatch_timeout::DEFAULT_TIMEOUT) is 120s in
        // production; `vox_test_agy_like_long_running`'s own configured
        // timeout is 2000ms (test-only). Sleep for longer than a plausible
        // "wrong, defaulted" timeout would allow but well inside its actual
        // 2000ms budget.
        let sleep_ms = 600u64;
        assert!(
            sleep_ms
                < crate::dispatch_timeout::timeout_for("vox_test_agy_like_long_running").as_millis()
                    as u64,
            "test sleep must stay under the tool's own configured timeout"
        );

        let res = handle_tool_call(
            &state,
            "vox_test_agy_like_long_running",
            json!({ "sleep_ms": sleep_ms }),
        )
        .await
        .expect("dispatch must not error");

        assert!(
            !crate::server_state::tool_json_envelope_is_error(&res),
            "agy-like long-running tool should have completed within its own exception timeout, got: {res}"
        );
        assert!(res.contains("agy-like delegation completed"));
    }

    /// RED test 3 (regression guard): a normal, fast tool completing well
    /// within its timeout is unaffected by the new outer guard.
    #[tokio::test]
    async fn fast_normal_tool_is_unaffected_by_the_outer_timeout() {
        let state = ServerState::new_test().await;
        let res = handle_tool_call(&state, "vox_skill_list", json!({}))
            .await
            .expect("dispatch must not error");
        assert!(
            !crate::server_state::tool_json_envelope_is_error(&res),
            "fast tool unexpectedly failed: {res}"
        );
    }
}
