use schemars::JsonSchema;
use serde::Deserialize;
use vox_orchestrator::types::TaskCategory;

use crate::llm_bridge::{McpChatModelResolution, resolve_mcp_chat_model_sync};
use crate::params::ToolResult;
use crate::server_state::ServerState;

const REM_MODEL_CATEGORY: &str = "Use a known `task_category` (parsing, typechecking, debugging, research, testing, codegen, review) or seed the model registry.";
const REM_MODEL_REGISTRY: &str =
    "Call `list_models` and pass a `model_id` that exists in the orchestrator registry.";
const REM_LOCK_POISON: &str =
    "Retry; if the error persists, restart the MCP server to clear a poisoned async lock.";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListModelsParams {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SuggestModelParams {
    pub task_category: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetModelParams {
    pub agent_id: u64,
    pub model_id: String,
}

pub async fn list_models(state: &ServerState, _params: ListModelsParams) -> String {
    let orch = &state.orchestrator;
    let handle = orch.models_handle();
    let models = vox_orchestrator::sync_lock::rw_read(&*handle).list_models();
    ToolResult::ok(models).to_json()
}

pub async fn suggest_model(state: &ServerState, params: SuggestModelParams) -> String {
    let orch = &state.orchestrator;

    // Parse task_category from string
    let category = match params.task_category.to_lowercase().as_str() {
        "parsing" => TaskCategory::Parsing,
        "typechecking" => TaskCategory::TypeChecking,
        "debugging" => TaskCategory::Debugging,
        "research" => TaskCategory::Research,
        "testing" => TaskCategory::Testing,
        "codegen" => TaskCategory::CodeGen,
        "review" => TaskCategory::Review,
        "general" | "ars" | "planning" => TaskCategory::General,
        "inter_agent" => TaskCategory::InterAgent,
        "tool_orchestration" => TaskCategory::ToolOrchestration,
        "visus" | "vision" | "gui" => TaskCategory::Visus,
        _ => {
            return ToolResult::<String>::err_with_remediation(
                "Unknown task_category",
                REM_MODEL_CATEGORY,
            )
            .to_json();
        }
    };

    let complexity = match category {
        TaskCategory::Parsing | TaskCategory::TypeChecking => 3,
        TaskCategory::Testing | TaskCategory::Debugging => 5,
        TaskCategory::CodeGen | TaskCategory::Review | TaskCategory::CodeEffortJudge => 6,
        TaskCategory::Research => 8,
        TaskCategory::General => 5,
        TaskCategory::Planning => 8,
        TaskCategory::Ars => 9,
        TaskCategory::InterAgent => 2,
        TaskCategory::ToolOrchestration => 5,
        TaskCategory::Visus => 8,
    };
    let resolution = McpChatModelResolution {
        allow_cheapest_fallback: true,
        complexity,
        task_category: category,
        free_tier_latency_critical: false,
        free_tier_fill_in_middle: false,
        enforce_free_tier_only: false,
        context_fill_ratio: None,
    };
    match resolve_mcp_chat_model_sync(orch, "", None, resolution) {
        Ok((model, _is_free)) => ToolResult::ok(model).to_json(),
        Err(_) => ToolResult::<String>::err_with_remediation(
            "No suitable model found for category",
            REM_MODEL_CATEGORY,
        )
        .to_json(),
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetActiveMcpModelParams {
    pub model_id: String,
}

/// Persist the user's preferred MCP inline-chat model (OpenRouter id, Gemini id, Ollama tag, …).
pub async fn set_active_mcp_chat_model(
    state: &ServerState,
    params: SetActiveMcpModelParams,
) -> String {
    let mut lock = match crate::sync_poison::poison_rw_write(
        state.mcp_chat_model_override.write(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g,
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e.to_string(), REM_LOCK_POISON)
                .to_json();
        }
    };
    if params.model_id.is_empty() {
        *lock = None;
        ToolResult::ok("cleared active MCP chat model override").to_json()
    } else {
        let id = params.model_id.clone();
        *lock = Some(id.clone());
        ToolResult::ok(format!("Active MCP chat model set to {id}")).to_json()
    }
}

/// Return the active MCP chat model override, if any.
pub async fn get_active_mcp_chat_model(state: &ServerState) -> String {
    let id = match crate::sync_poison::poison_rw_read(
        state.mcp_chat_model_override.read(),
        "mcp_chat_model_override",
    ) {
        Ok(g) => g.clone(),
        Err(e) => {
            return ToolResult::<String>::err_with_remediation(e.to_string(), REM_LOCK_POISON)
                .to_json();
        }
    };
    ToolResult::ok(id.unwrap_or_default()).to_json()
}

pub async fn set_model(state: &ServerState, params: SetModelParams) -> String {
    let orch = &state.orchestrator;

    let handle = orch.models_handle();
    if vox_orchestrator::sync_lock::rw_read(&*handle)
        .get(&params.model_id)
        .is_some()
    {
        vox_orchestrator::sync_lock::rw_write(&*handle)
            .set_override(params.agent_id, params.model_id.clone());
        ToolResult::ok(format!(
            "Successfully overridden model to {} for agent {}",
            params.model_id, params.agent_id
        ))
        .to_json()
    } else {
        ToolResult::<String>::err_with_remediation(
            format!("Model {} not found in registry", params.model_id),
            REM_MODEL_REGISTRY,
        )
        .to_json()
    }
}
