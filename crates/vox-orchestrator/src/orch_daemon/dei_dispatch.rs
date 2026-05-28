//! DeI JSON-line RPC methods (`ai.*`, `config.get`) for `vox-orchestrator-d` stdio/TCP transport.
//!
//! These method ids are shared with `vox-cli` / `vox-mcp` ([`vox_foundation::protocol::dei_method`]).

use std::sync::Arc;

use serde_json::{Value, json};
use vox_foundation::protocol::dei_method;

use super::{response_err, response_result};
use crate::Orchestrator;
use crate::orchestrator::task_dispatch::submit::dei_plan_materialize;
use vox_foundation::protocol::{DispatchRequest, DispatchResponse};

/// Dispatch `ai.*` / `config.get` when the incoming [`DispatchRequest::method`] matches DeI.
pub async fn try_dispatch_dei(
    repository_id: &str,
    orch: Arc<Orchestrator>,
    req: &DispatchRequest,
) -> Option<DispatchResponse> {
    match req.method.as_str() {
        dei_method::AI_PLAN_NEW => Some(handle_plan_new(&req.id, orch, &req.params).await),
        dei_method::AI_PLAN_REPLAN => Some(handle_plan_replan(&req.id, orch, &req.params).await),
        dei_method::AI_PLAN_STATUS => Some(handle_plan_status(&req.id, orch, &req.params).await),
        dei_method::AI_PLAN_EXECUTE => Some(handle_plan_execute(&req.id, orch, &req.params).await),
        dei_method::AI_CHECK => Some(handle_ai_check(&req.id, &req.params).await),
        dei_method::AI_FIX => Some(handle_ai_fix(&req.id, &req.params).await),
        dei_method::AI_REVIEW => Some(handle_ai_review(&req.id, &req.params).await),
        dei_method::AI_GENERATE => Some(handle_ai_generate(&req.id, &req.params).await),
        dei_method::CONFIG_GET => {
            Some(handle_config_get(&req.id, repository_id, orch, &req.params))
        }
        _ => None,
    }
}

async fn handle_plan_new(id: &str, orch: Arc<Orchestrator>, params: &Value) -> DispatchResponse {
    let goal = match params.get("goal").and_then(|v| v.as_str()) {
        Some(g) if !g.trim().is_empty() => g.to_string(),
        _ => return response_err(id, "params.goal (non-empty string) required"),
    };
    let origin_session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let scope_files: Vec<String> = params
        .get("scope_files")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    match dei_plan_materialize::dei_plan_new_json(&orch, goal, origin_session_id, scope_files).await
    {
        Ok(v) => response_result(id, v),
        Err(e) => response_err(id, format!("{e}")),
    }
}

async fn handle_plan_replan(id: &str, orch: Arc<Orchestrator>, params: &Value) -> DispatchResponse {
    let session_id = match params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return response_err(id, "params.session_id required"),
    };
    let delta = match params.get("delta_hint").and_then(|v| v.as_str()) {
        Some(d) if !d.is_empty() => d.to_string(),
        _ => return response_err(id, "params.delta_hint required"),
    };
    let Some(db) = orch.db() else {
        return response_err(
            id,
            "Codex DB not attached to orchestrator; cannot replan without persistence",
        );
    };
    let row = match db.get_plan_session_by_id(&session_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return response_err(id, format!("unknown plan session_id {session_id}")),
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let new_goal = format!("{}\n\n# Replan context\n{}", row.goal_text, delta);
    let next_ver = row.current_version + 1;
    if let Err(e) = db
        .append_plan_version(
            &session_id,
            next_ver,
            Some(row.current_version),
            Some("replan"),
            Some(&serde_json::json!({ "delta": delta }).to_string()),
        )
        .await
    {
        return response_err(id, format!("append_plan_version: {e}"));
    }
    match dei_plan_materialize::dei_plan_rematerialize_existing(
        &orch,
        &session_id,
        next_ver,
        new_goal,
        Vec::new(),
    )
    .await
    {
        Ok(v) => {
            let mut out = v;
            if let Some(obj) = out.as_object_mut() {
                obj.insert("replan_version".into(), json!(next_ver));
            }
            response_result(id, out)
        }
        Err(e) => response_err(id, format!("{e}")),
    }
}

async fn handle_plan_status(id: &str, orch: Arc<Orchestrator>, params: &Value) -> DispatchResponse {
    let session_id = match params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return response_err(id, "params.session_id required"),
    };
    let Some(db) = orch.db() else {
        return response_err(id, "Codex DB not attached");
    };
    let row = match db.get_plan_session_by_id(&session_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return response_err(id, format!("unknown session {session_id}")),
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let ver = match db.load_plan_head(&session_id).await {
        Ok(Some(v)) => v,
        Ok(None) => row.current_version,
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let nodes = match db.load_plan_nodes_with_status(&session_id, ver).await {
        Ok(n) => n,
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let mut pending = 0_u64;
    let mut completed = 0_u64;
    let mut failed = 0_u64;
    let mut in_progress = 0_u64;
    let mut skipped = 0_u64;
    for n in &nodes {
        match n.status.as_str() {
            "pending" | "queued" => pending += 1,
            "completed" | "done" => completed += 1,
            "failed" | "error" => failed += 1,
            "in_progress" | "running" => in_progress += 1,
            "skipped" => skipped += 1,
            _ => pending += 1,
        }
    }
    response_result(
        id,
        json!({
            "session_id": session_id,
            "goal": row.goal_text,
            "execution_mode": "",
            "latest_version": ver,
            "step_counts": {
                "pending": pending,
                "completed": completed,
                "failed": failed,
                "in_progress": in_progress,
                "skipped": skipped,
            },
            "recent_events": [],
        }),
    )
}

async fn handle_plan_execute(
    id: &str,
    orch: Arc<Orchestrator>,
    params: &Value,
) -> DispatchResponse {
    let session_id = match params.get("session_id").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return response_err(id, "params.session_id required"),
    };
    let Some(db) = orch.db() else {
        return response_err(id, "Codex DB not attached");
    };
    let origin = match db.get_plan_session_by_id(&session_id).await {
        Ok(Some(r)) => r.origin_session_id.clone(),
        Ok(None) => None,
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let ver = match db.load_plan_head(&session_id).await {
        Ok(Some(v)) => v,
        Ok(None) => return response_err(id, "plan has no head version"),
        Err(e) => return response_err(id, format!("db: {e}")),
    };
    let ver_u32: u32 = ver.clamp(1, i64::from(u32::MAX)) as u32;
    let tenant_id = params
        .get("tenant_id")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    match crate::planning::schedule::enqueue_runnable_plan_nodes(
        orch.as_ref(),
        &session_id,
        ver_u32,
        origin,
        tenant_id,
    )
    .await
    {
        Ok(ids) => {
            let arr: Vec<Value> = ids.into_iter().map(|t| json!(t.0)).collect();
            response_result(id, json!({ "task_ids": arr, "ok": true }))
        }
        Err(e) => response_err(id, format!("{e}")),
    }
}

async fn handle_ai_check(id: &str, params: &Value) -> DispatchResponse {
    let path = match params.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => return response_err(id, "params.path required"),
    };
    let root = if path.is_file() {
        path.parent().unwrap_or_else(|| std::path::Path::new("."))
    } else {
        path.as_path()
    };
    let cfg = vox_code_audit::ToestubConfig {
        roots: vec![root.to_path_buf()],
        min_severity: vox_code_audit::Severity::Info,
        format: vox_code_audit::OutputFormat::Json,
        run_mode: vox_code_audit::ToestubRunMode::Audit,
        ..Default::default()
    };
    let engine = vox_code_audit::ToestubEngine::new(cfg);
    let res = engine.run();
    let findings: Vec<Value> = res
        .findings
        .iter()
        .map(|f| {
            json!({
                "rule": f.rule_id,
                "severity": format!("{:?}", f.severity),
                "message": f.message,
                "path": f.file.display().to_string(),
            })
        })
        .collect();
    response_result(
        id,
        json!({
            "ok": !res.has_errors(),
            "files_scanned": res.files_scanned,
            "findings": findings,
            "repository_id": "",
        }),
    )
}

async fn handle_ai_fix(id: &str, params: &Value) -> DispatchResponse {
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let code = params
        .get("code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let err_blob = params
        .get("errors")
        .or_else(|| params.get("error"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prompt = format!(
        "Fix the following source file. Path: {path}\n\nCompiler/linter output:\n{err_blob}\n\nSource:\n{code}\n\nReturn only the corrected full file contents, no markdown fences."
    );
    let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
    match client.generate(&prompt).await {
        Ok(text) => response_result(id, json!({ "fixed": text, "path": path })),
        Err(e) => response_err(id, format!("{e}")),
    }
}

async fn handle_ai_review(id: &str, params: &Value) -> DispatchResponse {
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let diff = params.get("diff").and_then(|v| v.as_str()).unwrap_or("");
    let prompt = format!(
        "Review this change. Path: {path}\n\nDiff or excerpt:\n{diff}\n\nSummarize risks and testing gaps in <= 20 lines."
    );
    let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
    match client.generate(&prompt).await {
        Ok(text) => response_result(id, json!({ "review": text })),
        Err(e) => response_err(id, format!("{e}")),
    }
}

async fn handle_ai_generate(id: &str, params: &Value) -> DispatchResponse {
    let prompt = match params.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p,
        _ => return response_err(id, "params.prompt required"),
    };
    let client = vox_gamify::ai::FreeAiClient::auto_discover().await;
    match client.generate(prompt).await {
        Ok(text) => response_result(id, json!({ "text": text })),
        Err(e) => response_err(id, format!("{e}")),
    }
}

fn handle_config_get(
    id: &str,
    repository_id: &str,
    orch: Arc<Orchestrator>,
    params: &Value,
) -> DispatchResponse {
    let _key = params.get("key").and_then(|v| v.as_str());
    let cfg = crate::sync_lock::rw_read(&*orch.config).clone();
    response_result(
        id,
        json!({
            "repository_id": repository_id,
            "max_agents": cfg.max_agents,
            "planning_enabled": cfg.planning_enabled,
            "planning_llm_synthesis_enabled": cfg.planning_llm_synthesis_enabled,
            "research_model_enabled": cfg.research_model_enabled,
        }),
    )
}
