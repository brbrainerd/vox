//! Bounded LLM assist for SCIENTIA — JSON-only suggestions, [`TaskCategory::Research`] routing.

use crate::params::ToolResult;
use crate::server_state::ServerState;
use schemars::JsonSchema;
use serde::Deserialize;
use vox_orchestrator::types::TaskCategory;

use crate::chat_model_resolve::resolve_chat_llm_model;
use crate::llm_bridge::{McpChatModelResolution, McpInferRouting, mcp_infer_completion};
use crate::text_normalization::strip_json_codeblock_fence;

use super::common::{
    REM_PUBLICATION_ID, REM_SCIENTIA_DB, no_voxdb_tool_string, publication_manifest_from_row,
};

const REM_MCP_MODEL_LOCK: &str = "Retry after releasing the MCP chat model override lock.";
const REM_MCP_MODEL_RESOLVE: &str = "Run `list_models`, ensure Ollama/API routes work, and check `vox secrets doctor` for inference secrets.";
const REM_LLM_COMPLETION: &str = "Check inference logs, rate limits, and backend health; verify API keys via `vox secrets doctor`.";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VoxScientiaAssistSuggestionsParams {
    pub publication_id: String,
    /// When false, return heuristic structured gaps only (no HTTP inference).
    #[serde(default = "default_use_llm")]
    pub use_llm: bool,
}

fn default_use_llm() -> bool {
    true
}

pub async fn vox_scientia_assist_suggestions(
    state: &ServerState,
    params: VoxScientiaAssistSuggestionsParams,
) -> String {
    let Some(db) = &state.db else {
        return no_voxdb_tool_string();
    };
    let row = match db.get_publication_manifest(&params.publication_id).await {
        Ok(r) => r,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("DB error: {e}"),
                REM_SCIENTIA_DB,
            )
            .to_json();
        }
    };
    let Some(row) = row else {
        return ToolResult::<String>::err_with_remediation(
            "publication not found",
            REM_PUBLICATION_ID,
        )
        .to_json();
    };
    let manifest = publication_manifest_from_row(&row);
    let evidence =
        vox_publisher::scientia_evidence::parse_scientia_evidence(row.metadata_json.as_deref())
            .unwrap_or_default();
    let scientia_h = vox_publisher::scientia_heuristics::ScientiaHeuristics::load_from_repo_root(
        &state.repository.root,
    );
    let heuristic_rank = vox_publisher::scientia_discovery::rank_candidate_heuristics(
        params.publication_id.as_str(),
        row.source_ref.as_deref(),
        &evidence,
        &scientia_h,
        None,
    );
    let completion = vox_publisher::scientia_discovery::manifest_completion_report(&manifest);
    let evidence_json = serde_json::to_string(&evidence).unwrap_or_default();
    let heuristic_payload = serde_json::json!({
        "schema_version": 1,
        vox_publisher::scientia_evidence::SCIENTIA_LABEL_MACHINE_SUGGESTED: true,
        vox_publisher::scientia_evidence::SCIENTIA_LABEL_REQUIRES_HUMAN_REVIEW: true,
        vox_publisher::scientia_evidence::SCIENTIA_LABEL_SOURCE_GROUNDED: true,
        "discovery_rank": heuristic_rank,
        "manifest_completion": completion,
        "evidence_excerpt": evidence_json,
    });

    if !params.use_llm {
        return ToolResult::ok(heuristic_payload).to_json();
    }

    let system_prompt = "You are a scientific publication workflow assistant. Output ONLY valid JSON (no markdown fences, no commentary). Keys: checklist_items (array of {code, summary, blocking: boolean}), gap_notes (array of string), refusal_reasons (array of string if you cannot ground a item). Never invent citations, novelty claims, or benchmark results. Use only MANIFEST_SUMMARY and EVIDENCE_JSON.";

    let user_prompt = format!(
        "MANIFEST_SUMMARY:\n{}\n\nEVIDENCE_JSON:\n{}\n\nHEURISTIC:\n{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "title": manifest.title,
            "author": manifest.author,
            "has_abstract": manifest.abstract_text.as_ref().is_some_and(|s| !s.trim().is_empty()),
            "source_ref": manifest.source_ref,
        }))
        .unwrap_or_default(),
        evidence_json,
        serde_json::to_string_pretty(&heuristic_payload).unwrap_or_default(),
    );

    let resolution_template = McpChatModelResolution {
        complexity: 4,
        task_category: TaskCategory::Research,
        allow_cheapest_fallback: true,
        ..Default::default()
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
    let (model, free_only) = match resolve_chat_llm_model(
        state,
        &user_prompt,
        resolution_template.clone(),
        None,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(
                format!("No model: {e}"),
                REM_MCP_MODEL_RESOLVE,
            )
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
        user_id: None,
    };

    let (raw, _model_used, _tok) = match mcp_infer_completion(
        state,
        model,
        "vox_scientia_assist_suggestions",
        system_prompt,
        &routing,
        1024,
        0.2,
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

    let stripped = strip_json_codeblock_fence(raw.trim());
    let parsed: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult::ok(serde_json::json!({
                "heuristic": heuristic_payload,
                "llm_parse_error": e.to_string(),
                "llm_raw_excerpt": stripped.chars().take(512).collect::<String>(),
            }))
            .to_json();
        }
    };

    ToolResult::ok(serde_json::json!({
        "heuristic": heuristic_payload,
        "llm_suggestions": parsed,
        vox_publisher::scientia_evidence::SCIENTIA_LABEL_REQUIRES_HUMAN_REVIEW: true,
    }))
    .to_json()
}
