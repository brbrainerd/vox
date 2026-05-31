use super::params::{ANTI_LAZINESS_RIDER, InlineEditParams, InlineEditResult};
use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::chat_socrates_meta::{
    clarification_turn_for_session, mcp_questioning_session_key, socrates_surface_tags,
    socrates_system_rider, socrates_tool_meta, spawn_questioning_trace_from_socrates,
    spawn_socrates_telemetry_with_meta,
};
use crate::llm_bridge::{
    McpChatModelResolution, McpInferRouting, clamp_http_max_output_tokens, mcp_infer_completion,
};
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_MCP_MODEL_LOCK: &str =
    "Retry; restart the MCP server if `mcp_chat_model_override` stays poisoned.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

/// Perform an inline edit on a range in a file.
/// The editor sends the current text; Rust queries the LLM and returns the replacement.
pub async fn inline_edit(state: &ServerState, params: InlineEditParams) -> String {
    let language = params.language.as_deref().unwrap_or("text");
    let context_before = params.context_before.as_deref().unwrap_or("");
    let context_after = params.context_after.as_deref().unwrap_or("");

    let user_prompt = format!(
        r"You are an expert {language} programmer. Edit the following code snippet as instructed.

INSTRUCTION: {prompt}

CONTEXT BEFORE (do not modify):
```{language}
{context_before}
```

CODE TO EDIT (lines {start_line}-{end_line} of file `{file}`):
```{language}
{current_text}
```

CONTEXT AFTER (do not modify):
```{language}
{context_after}
```

OUTPUT RULES:
- Output ONLY the replacement code for lines {start_line}-{end_line}.
- Do NOT include context_before or context_after.
- Do NOT wrap output in markdown fences — output raw code only.
- Preserve indentation consistent with context_before.
- Do NOT add placeholder comments or TODOs.",
        prompt = params.prompt,
        file = params.file,
        start_line = params.start_line,
        end_line = params.end_line,
        current_text = params.current_text,
    );

    let pol = state.orchestrator_config.effective_socrates_policy();
    let system_prompt = format!(
        "You are an expert inline code editor. You output ONLY replacement code, no markdown fences, no explanation.{}\n{}",
        ANTI_LAZINESS_RIDER,
        socrates_system_rider(&pol)
    );

    let resolution_template = McpChatModelResolution {
        allow_cheapest_fallback: true,
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
                e.to_string(),
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
    let max_tokens = clamp_http_max_output_tokens(model.max_tokens);
    let routing = McpInferRouting {
        user_prompt: &user_prompt,
        sticky_model_pref: pref.as_deref(),
        resolution_template,
        free_only,
        allow_cloud_ollama_fallback: true,
        selection_rationale: None,
        user_id: params.session_id.as_deref(),
    };

    let inline_llm_started = std::time::Instant::now();
    let (replacement, model_used, tokens) = match mcp_infer_completion(
        state,
        model,
        "mcp_inline_edit",
        &system_prompt,
        &routing,
        max_tokens,
        0.3,
        params.temperature,
        params.top_p,
        params.json_mode,
        params.attachment_manifest,
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

    let inline_session_key =
        mcp_questioning_session_key(state, "vox_inline_edit", params.session_id.as_deref());
    state.record_questioning_attention_spend(
        &inline_session_key,
        inline_llm_started.elapsed().as_millis() as u64,
    );

    let result = InlineEditResult {
        replacement: replacement.trim().to_string(),
        explanation: params.prompt.clone(),
        tokens,
        model_used,
    };

    let grounding = 0.66_f64;
    let session_key = inline_session_key;
    let turn = clarification_turn_for_session(state, &session_key).await;
    let (spent_att, max_att) = state.questioning_attention_bounds(&session_key);
    let soc = socrates_tool_meta(
        &pol,
        grounding,
        params.current_text.len() < 8,
        turn,
        spent_att,
        max_att,
        None,
    );
    spawn_socrates_telemetry_with_meta(
        state,
        "vox_inline_edit",
        soc.clone(),
        Some(result.model_used.clone()),
        Some(socrates_surface_tags(
            "inline_edit",
            &["interactive", "code_edit"],
        )),
    );
    spawn_questioning_trace_from_socrates(
        state,
        "vox_inline_edit",
        soc.clone(),
        Some(session_key.clone()),
        Some(params.prompt.clone()),
    );

    let mut v = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = v.as_object_mut() {
        obj.insert("socrates".to_string(), soc);
    }
    ToolResult::ok(v).to_json()
}
