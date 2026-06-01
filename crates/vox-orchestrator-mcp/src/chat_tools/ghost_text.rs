use super::params::{GhostTextParams, GhostTextResult};
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::chat_socrates_meta::{
    clarification_turn_for_session, mcp_questioning_session_key, socrates_surface_tags,
    socrates_system_rider, socrates_tool_meta, spawn_questioning_trace_from_socrates,
    spawn_socrates_telemetry_with_meta,
};
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, mcp_infer_completion};
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_MCP_MODEL_LOCK: &str =
    "Retry; restart the MCP server if `mcp_chat_model_override` stays poisoned.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

pub(crate) fn ghost_grounding_score(params: &GhostTextParams) -> f64 {
    let mut n = 0u32;
    if params.file_path.is_some() {
        n += 1;
    }
    if !params.prefix.trim().is_empty() {
        n += 1;
    }
    if !params.suffix.trim().is_empty() {
        n += 1;
    }
    (0.50 + 0.12 * f64::from(n.min(3))).min(0.88)
}

/// Handle the `vox_ghost_text` tool call.
///
/// Builds a fill-in-the-middle (FIM) prompt optimised for single-line editor
/// completions and routes it to the fastest available LLM. Targets p95 < 50 ms
/// time-to-first-token when using a local Ollama / Mens inference server.
pub async fn ghost_text(state: &ServerState, params: GhostTextParams) -> String {
    let language = params.language.as_deref().unwrap_or("vox");
    let file_hint = params
        .file_path
        .as_deref()
        .map(|p| format!("File: {p}\n"))
        .unwrap_or_default();
    let max_tokens = params.max_tokens.unwrap_or(128);

    // FIM-style prompt: give the model clear boundaries.
    let user_prompt = format!(
        r"{file_hint}Complete the following {language} code. Output ONLY the completion — no markdown, no explanation, no fences.

<|fim_prefix|>{prefix}<|fim_suffix|>{suffix}<|fim_middle|>",
        prefix = params.prefix,
        suffix = params.suffix,
    );

    let pol = state.orchestrator_config.effective_socrates_policy();
    let system_prompt = format!(
        "You are an expert {language} code completion engine. Produce only the missing code fragment that naturally continues the prefix. \
         Keep completions concise (typically 1-3 lines). Never repeat the prefix or suffix. Never add markdown.\n{}",
        socrates_system_rider(&pol)
    );

    let t0 = std::time::Instant::now();

    let resolution_template = McpChatModelResolution {
        complexity: 2,
        free_tier_latency_critical: true,
        free_tier_fill_in_middle: true,
        allow_cheapest_fallback: true,
        enforce_free_tier_only: true,
        ..Default::default()
    };
    let (model, free_only) = match resolve_chat_llm_model(
        state,
        &user_prompt,
        resolution_template.clone(),
        params.session_id.as_deref(),
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
    let pref = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e.to_string(), REM_MCP_MODEL_LOCK)
                .to_json();
        }
    };
    let routing = McpInferRouting {
        user_prompt: &user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        selection_rationale: None,
        user_id: params.session_id.as_deref(),
    };

    let (mut completion, model_used, tokens) = match mcp_infer_completion(
        state,
        model,
        "mcp_ghost_text",
        &system_prompt,
        &routing,
        max_tokens,
        0.2,
        params.temperature,
        params.top_p,
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

    let latency_ms = t0.elapsed().as_millis() as u64;

    let ghost_session_key =
        mcp_questioning_session_key(state, "vox_ghost_text", params.session_id.as_deref());
    state.record_questioning_attention_spend(&ghost_session_key, latency_ms);

    // Strip any accidental fence wrappers the model may emit.
    if let Some(inner) = completion
        .strip_prefix(&format!("```{language}"))
        .or_else(|| completion.strip_prefix("```"))
    {
        completion = inner
            .trim_start_matches('\n')
            .trim_end_matches("```")
            .trim_end()
            .to_string();
    }

    // Cap at max_tokens * 4 bytes as a rough UTF-8 token proxy.
    if completion.len() > max_tokens as usize * 4 {
        completion = completion[..max_tokens as usize * 4].to_string();
    }

    let result = GhostTextResult {
        completion: completion.trim().to_string(),
        model_used,
        tokens,
        latency_ms,
    };

    tracing::debug!(
        latency_ms,
        model = %result.model_used,
        "ghost_text: {} chars generated",
        result.completion.len()
    );

    let thin_context = params.prefix.len() + params.suffix.len() < 40;
    let grounding = ghost_grounding_score(&params);
    let session_key = ghost_session_key;
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(
        &pol,
        grounding,
        thin_context,
        turn,
        spent_att,
        max_att,
        None,
    );
    spawn_socrates_telemetry_with_meta(
        state,
        "vox_ghost_text",
        soc.clone(),
        Some(result.model_used.clone()),
        Some(socrates_surface_tags(
            "code_completion",
            &["interactive", "code_generation"],
        )),
    );
    spawn_questioning_trace_from_socrates(
        state,
        "vox_ghost_text",
        soc.clone(),
        Some(session_key.clone()),
        None,
    );
    let mut v = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("socrates".to_string(), soc);
    }
    ToolResult::ok(v).to_json()
}
